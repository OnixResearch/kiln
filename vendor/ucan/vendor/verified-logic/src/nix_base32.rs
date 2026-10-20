//! Verified Nix Base32 encoding.
//!
//! Matches upstream `BaseNix32`: bits are grouped in 5-bit chunks starting at
//! the low bits of the first byte, and output characters are emitted from the
//! highest 5-bit chunk down to the lowest.
//!
//! Important: upstream decode accepts non-canonical strings like `"0"` and
//! `"z"`; canonical round-trips are guaranteed for `encode(bytes)` outputs.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

pub closed spec fn alphabet_spec() -> Seq<u8> {
    seq![
        48u8, 49u8, 50u8, 51u8, 52u8, 53u8, 54u8, 55u8,
        56u8, 57u8, 97u8, 98u8, 99u8, 100u8, 102u8, 103u8,
        104u8, 105u8, 106u8, 107u8, 108u8, 109u8, 110u8, 112u8,
        113u8, 114u8, 115u8, 118u8, 119u8, 120u8, 121u8, 122u8,
    ]
}

// r[depends nix-base32.output-length]
/// Spec: encoded length in characters.
pub open spec fn encoded_length_spec(input_len: nat) -> nat {
    (input_len * 8 + 4) / 5
}

pub closed spec fn bit_at_padded_spec(bytes: Seq<u8>, pos: nat) -> u8 {
    if pos < bytes.len() * 8 {
        ((((bytes[(pos / 8) as int] as u64) >> ((pos % 8) as u64)) & 1u64) as u8)
    } else {
        0u8
    }
}

pub closed spec fn digit_from_bit_spec(bytes: Seq<u8>, start: nat) -> u8 {
    ((bit_at_padded_spec(bytes, start) as u64)
        | ((bit_at_padded_spec(bytes, start + 1) as u64) << 1u64)
        | ((bit_at_padded_spec(bytes, start + 2) as u64) << 2u64)
        | ((bit_at_padded_spec(bytes, start + 3) as u64) << 3u64)
        | ((bit_at_padded_spec(bytes, start + 4) as u64) << 4u64)) as u8
}

pub closed spec fn encode_char_spec(bytes: Seq<u8>, out_len: nat, idx: nat) -> u8
    recommends idx < out_len
{
    alphabet_spec()[digit_from_bit_spec(bytes, ((out_len - 1 - idx) * 5) as nat) as int]
}

pub closed spec fn encode_prefix_spec(bytes: Seq<u8>, out_len: nat, upto: nat) -> Seq<u8>
    recommends upto <= out_len
{
    Seq::new(upto, |i: int| encode_char_spec(bytes, out_len, i as nat))
}

// decode_char_spec cases
pub closed spec fn decode_char_spec(c: u8) -> Option<u8> {
    if c == 48u8 { Some(0u8) }
    else if c == 49u8 { Some(1u8) }
    else if c == 50u8 { Some(2u8) }
    else if c == 51u8 { Some(3u8) }
    else if c == 52u8 { Some(4u8) }
    else if c == 53u8 { Some(5u8) }
    else if c == 54u8 { Some(6u8) }
    else if c == 55u8 { Some(7u8) }
    else if c == 56u8 { Some(8u8) }
    else if c == 57u8 { Some(9u8) }
    else if c == 97u8 { Some(10u8) }
    else if c == 98u8 { Some(11u8) }
    else if c == 99u8 { Some(12u8) }
    else if c == 100u8 { Some(13u8) }
    else if c == 102u8 { Some(14u8) }
    else if c == 103u8 { Some(15u8) }
    else if c == 104u8 { Some(16u8) }
    else if c == 105u8 { Some(17u8) }
    else if c == 106u8 { Some(18u8) }
    else if c == 107u8 { Some(19u8) }
    else if c == 108u8 { Some(20u8) }
    else if c == 109u8 { Some(21u8) }
    else if c == 110u8 { Some(22u8) }
    else if c == 112u8 { Some(23u8) }
    else if c == 113u8 { Some(24u8) }
    else if c == 114u8 { Some(25u8) }
    else if c == 115u8 { Some(26u8) }
    else if c == 118u8 { Some(27u8) }
    else if c == 119u8 { Some(28u8) }
    else if c == 120u8 { Some(29u8) }
    else if c == 121u8 { Some(30u8) }
    else if c == 122u8 { Some(31u8) }
    else { None }
}

pub closed spec fn zero_bytes_spec(n: nat) -> Seq<u8> {
    Seq::new(n, |i: int| 0u8)
}

pub closed spec fn extend_to_spec(acc: Seq<u8>, len: nat) -> Seq<u8>
    recommends acc.len() <= len
{
    acc + zero_bytes_spec((len - acc.len()) as nat)
}

pub closed spec fn or_extend_at_spec(acc: Seq<u8>, idx: nat, val: u8) -> Seq<u8> {
    let base = if acc.len() <= idx {
        extend_to_spec(acc, idx + 1)
    } else {
        acc
    };
    base.update(idx as int, (((base[idx as int] as u64) | (val as u64)) as u8))
}

pub closed spec fn decode_low_spec(bit: nat, digit: u8) -> u8 {
    (((digit as u64) << ((bit % 8) as u64)) & 0xffu64) as u8
}

pub closed spec fn decode_high_spec(bit: nat, digit: u8) -> u8 {
    let j = bit % 8;
    if j == 0 { 0u8 } else { ((digit as u64) >> (8u64 - j as u64)) as u8 }
}

pub closed spec fn decode_step_spec(acc: Seq<u8>, n: nat, digit: u8) -> Seq<u8> {
    let bit = n * 5;
    let i = bit / 8;
    let low = decode_low_spec(bit, digit);
    let acc1 = or_extend_at_spec(acc, i, low);
    let high = decode_high_spec(bit, digit);
    if high == 0u8 {
        acc1
    } else {
        or_extend_at_spec(acc1, i + 1, high)
    }
}

pub closed spec fn decode_prefix_spec(chars: Seq<u8>, upto: nat) -> Option<Seq<u8>>
    recommends upto <= chars.len()
    decreases upto
{
    if upto == 0 {
        Some(Seq::<u8>::empty())
    } else {
        match decode_prefix_spec(chars, (upto - 1) as nat) {
            None => None,
            Some(acc) => {
                let c = chars[(chars.len() - upto) as int];
                match decode_char_spec(c) {
                    None => None,
                    Some(d) => Some(decode_step_spec(acc, (upto - 1) as nat, d)),
                }
            }
        }
    }
}

pub closed spec fn reconstruct_prefix_spec(bytes: Seq<u8>, upto: nat) -> Seq<u8>
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else {
        decode_step_spec(
            reconstruct_prefix_spec(bytes, (upto - 1) as nat),
            (upto - 1) as nat,
            digit_from_bit_spec(bytes, ((upto - 1) * 5) as nat),
        )
    }
}

// r[depends nix-base32.encode]
/// Spec: encode bytes using Nix Base32.
pub open spec fn encode_spec(bytes: Seq<u8>) -> Seq<u8> {
    let out_len = encoded_length_spec(bytes.len());
    encode_prefix_spec(bytes, out_len, out_len)
}

// r[depends nix-base32.decode]
/// Spec: decode a Nix Base32 string. Returns None for invalid characters.
pub open spec fn decode_spec(chars: Seq<u8>) -> Option<Seq<u8>> {
    decode_prefix_spec(chars, chars.len())
}

// r[verify nix-base32.output-length]
/// Encoded length follows ceiling(bits / 5).
pub proof fn encode_output_length(bytes: Seq<u8>)
    ensures encode_spec(bytes).len() == encoded_length_spec(bytes.len())
{}

proof fn bit_at_padded_bounded(bytes: Seq<u8>, pos: nat)
    ensures bit_at_padded_spec(bytes, pos) <= 1u8
{
    if pos < bytes.len() * 8 {
        let byte = bytes[(pos / 8) as int];
        let shift: u64 = (pos % 8) as u64;
        assert(((((byte as u64) >> shift) & 1u64) as u8) <= 1u8) by(bit_vector);
    }
}

proof fn digit_from_bit_bounded(bytes: Seq<u8>, start: nat)
    ensures digit_from_bit_spec(bytes, start) < 32
{
    let b0 = bit_at_padded_spec(bytes, start);
    let b1 = bit_at_padded_spec(bytes, start + 1);
    let b2 = bit_at_padded_spec(bytes, start + 2);
    let b3 = bit_at_padded_spec(bytes, start + 3);
    let b4 = bit_at_padded_spec(bytes, start + 4);
    bit_at_padded_bounded(bytes, start);
    bit_at_padded_bounded(bytes, start + 1);
    bit_at_padded_bounded(bytes, start + 2);
    bit_at_padded_bounded(bytes, start + 3);
    bit_at_padded_bounded(bytes, start + 4);
    assert(((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) < 32u64) by(bit_vector)
        requires
            b0 <= 1u8,
            b1 <= 1u8,
            b2 <= 1u8,
            b3 <= 1u8,
            b4 <= 1u8,
    ;
}

proof fn extend_to_len(acc: Seq<u8>, len: nat)
    requires acc.len() <= len
    ensures extend_to_spec(acc, len).len() == len
{}

proof fn extend_to_same_len(acc: Seq<u8>)
    ensures extend_to_spec(acc, acc.len()) == acc
{}

proof fn or_extend_at_len(acc: Seq<u8>, idx: nat, val: u8)
    ensures or_extend_at_spec(acc, idx, val).len() == if acc.len() <= idx { idx + 1 } else { acc.len() }
{
    if acc.len() <= idx {
        extend_to_len(acc, idx + 1);
    }
}

proof fn or_extend_at_within_len(acc: Seq<u8>, len: nat, idx: nat, val: u8)
    requires
        acc.len() <= len,
        idx < len,
    ensures
        or_extend_at_spec(acc, idx, val).len() <= len,
{
    or_extend_at_len(acc, idx, val);
    if acc.len() <= idx {
        assert(or_extend_at_spec(acc, idx, val).len() == idx + 1);
        assert(idx + 1 <= len);
    } else {
        assert(or_extend_at_spec(acc, idx, val).len() == acc.len());
    }
}

proof fn encoded_length_ceil(len: nat)
    ensures
        encoded_length_spec(len) * 5 >= len * 8,
        len != 0 ==> (encoded_length_spec(len) - 1) * 5 < len * 8,
        encoded_length_spec(len) == 0 <==> len == 0,
{
    assert(encoded_length_spec(len) * 5 >= len * 8) by(nonlinear_arith);
    if len != 0 {
        assert((encoded_length_spec(len) - 1) * 5 < len * 8) by(nonlinear_arith);
    }
    assert(encoded_length_spec(len) == 0 <==> len == 0) by(nonlinear_arith);
}

proof fn chunk_start_in_bounds(len: nat, n: nat)
    requires
        len > 0,
        n < encoded_length_spec(len),
    ensures
        n * 5 < len * 8,
        (n * 5) / 8 < len,
{
    let out_len = encoded_length_spec(len);
    encoded_length_ceil(len);
    assert(out_len > 0);
    assert((out_len - 1) * 5 < len * 8);
    assert(n as int <= out_len as int - 1) by(nonlinear_arith)
        requires n < out_len, out_len > 0;
    assert((n * 5) as int <= ((out_len - 1) * 5) as int) by(nonlinear_arith)
        requires n as int <= out_len as int - 1;
    assert(n * 5 < len * 8) by(nonlinear_arith)
        requires
            (n * 5) as int <= ((out_len - 1) * 5) as int,
            (out_len - 1) * 5 < len * 8;
    assert((n * 5) / 8 < len) by(nonlinear_arith)
        requires n * 5 < len * 8;
}

proof fn div_mod_decompose_8(x: nat)
    ensures x == 8 * (x / 8) + (x % 8)
{
    assert(x == 8 * (x / 8) + (x % 8)) by(nonlinear_arith);
}

proof fn bit_at_padded_zero_beyond_end(bytes: Seq<u8>, pos: nat)
    requires pos >= bytes.len() * 8
    ensures bit_at_padded_spec(bytes, pos) == 0u8
{}

proof fn decode_high_zero_when_no_spill(bit: nat, digit: u8)
    requires
        digit < 32,
        bit % 8 <= 3,
    ensures decode_high_spec(bit, digit) == 0u8
{
    let j = bit % 8;
    if j == 0 {
    } else if j == 1 {
        assert((digit as u64) >> 7u64 == 0u64) by(bit_vector)
            requires digit < 32u8;
    } else if j == 2 {
        assert((digit as u64) >> 6u64 == 0u64) by(bit_vector)
            requires digit < 32u8;
    } else {
        assert(j == 3);
        assert((digit as u64) >> 5u64 == 0u64) by(bit_vector)
            requires digit < 32u8;
    }
}

proof fn last_chunk_spill_zero(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        ((n * 5) / 8) + 1 == bytes.len(),
        4 <= (n * 5) % 8,
    ensures decode_high_spec(n * 5, digit_from_bit_spec(bytes, n * 5)) == 0u8
{
    let bit = n * 5;
    let j = bit % 8;
    let d = digit_from_bit_spec(bytes, bit);
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    digit_from_bit_bounded(bytes, bit);
    bit_at_padded_bounded(bytes, bit);
    bit_at_padded_bounded(bytes, bit + 1);
    bit_at_padded_bounded(bytes, bit + 2);
    bit_at_padded_bounded(bytes, bit + 3);
    bit_at_padded_bounded(bytes, bit + 4);
    div_mod_decompose_8(bit);
    assert(bit / 8 + 1 == bytes.len());
    assert(bit == 8 * (bytes.len() - 1) + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + j,
            bit / 8 + 1 == bytes.len(),
            bytes.len() > 0;
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    if j == 4 {
        assert(bit + 4 == bytes.len() * 8) by(nonlinear_arith)
            requires bit == 8 * (bytes.len() - 1) + j, j == 4, bytes.len() > 0;
        bit_at_padded_zero_beyond_end(bytes, bit + 4);
        assert(b4 == 0u8);
        assert((d as u64) >> 4u64 == 0u64) by(bit_vector)
            requires
                d == ((b0 as u64)
                    | ((b1 as u64) << 1u64)
                    | ((b2 as u64) << 2u64)
                    | ((b3 as u64) << 3u64)
                    | ((b4 as u64) << 4u64)) as u8,
                b0 <= 1u8,
                b1 <= 1u8,
                b2 <= 1u8,
                b3 <= 1u8,
                b4 == 0u8;
        assert((((d as u64) >> 4u64) as u8) == 0u8) by(bit_vector)
            requires (d as u64) >> 4u64 == 0u64;
    } else if j == 5 {
        assert(bit + 3 == bytes.len() * 8) by(nonlinear_arith)
            requires bit == 8 * (bytes.len() - 1) + j, j == 5, bytes.len() > 0;
        bit_at_padded_zero_beyond_end(bytes, bit + 3);
        bit_at_padded_zero_beyond_end(bytes, bit + 4);
        assert(b3 == 0u8);
        assert(b4 == 0u8);
        assert((d as u64) >> 3u64 == 0u64) by(bit_vector)
            requires
                d == ((b0 as u64)
                    | ((b1 as u64) << 1u64)
                    | ((b2 as u64) << 2u64)
                    | ((b3 as u64) << 3u64)
                    | ((b4 as u64) << 4u64)) as u8,
                b0 <= 1u8,
                b1 <= 1u8,
                b2 <= 1u8,
                b3 == 0u8,
                b4 == 0u8;
        assert((((d as u64) >> 3u64) as u8) == 0u8) by(bit_vector)
            requires (d as u64) >> 3u64 == 0u64;
    } else if j == 6 {
        assert(bit + 2 == bytes.len() * 8) by(nonlinear_arith)
            requires bit == 8 * (bytes.len() - 1) + j, j == 6, bytes.len() > 0;
        bit_at_padded_zero_beyond_end(bytes, bit + 2);
        bit_at_padded_zero_beyond_end(bytes, bit + 3);
        bit_at_padded_zero_beyond_end(bytes, bit + 4);
        assert(b2 == 0u8);
        assert(b3 == 0u8);
        assert(b4 == 0u8);
        assert((d as u64) >> 2u64 == 0u64) by(bit_vector)
            requires
                d == ((b0 as u64)
                    | ((b1 as u64) << 1u64)
                    | ((b2 as u64) << 2u64)
                    | ((b3 as u64) << 3u64)
                    | ((b4 as u64) << 4u64)) as u8,
                b0 <= 1u8,
                b1 <= 1u8,
                b2 == 0u8,
                b3 == 0u8,
                b4 == 0u8;
        assert((((d as u64) >> 2u64) as u8) == 0u8) by(bit_vector)
            requires (d as u64) >> 2u64 == 0u64;
    } else {
        assert(j == 7);
        assert(bit + 1 == bytes.len() * 8) by(nonlinear_arith)
            requires bit == 8 * (bytes.len() - 1) + j, j == 7, bytes.len() > 0;
        bit_at_padded_zero_beyond_end(bytes, bit + 1);
        bit_at_padded_zero_beyond_end(bytes, bit + 2);
        bit_at_padded_zero_beyond_end(bytes, bit + 3);
        bit_at_padded_zero_beyond_end(bytes, bit + 4);
        assert(b1 == 0u8);
        assert(b2 == 0u8);
        assert(b3 == 0u8);
        assert(b4 == 0u8);
        assert((d as u64) >> 1u64 == 0u64) by(bit_vector)
            requires
                d == ((b0 as u64)
                    | ((b1 as u64) << 1u64)
                    | ((b2 as u64) << 2u64)
                    | ((b3 as u64) << 3u64)
                    | ((b4 as u64) << 4u64)) as u8,
                b0 <= 1u8,
                b1 == 0u8,
                b2 == 0u8,
                b3 == 0u8,
                b4 == 0u8;
        assert((((d as u64) >> 1u64) as u8) == 0u8) by(bit_vector)
            requires (d as u64) >> 1u64 == 0u64;
    }
}

proof fn decode_char_of_alphabet_digit(d: u8)
    requires d < 32
    ensures decode_char_spec(alphabet_spec()[d as int]) == Some(d)
{}

proof fn decode_prefix_of_encode(bytes: Seq<u8>, upto: nat)
    requires upto <= encoded_length_spec(bytes.len())
    ensures decode_prefix_spec(encode_spec(bytes), upto) == Some(reconstruct_prefix_spec(bytes, upto))
    decreases upto
{
    let out_len = encoded_length_spec(bytes.len());
    if upto != 0 {
        decode_prefix_of_encode(bytes, (upto - 1) as nat);
        let c = encode_spec(bytes)[(out_len - upto) as int];
        assert(c == encode_char_spec(bytes, out_len, (out_len - upto) as nat));
        assert(((out_len - 1 - (out_len - upto)) * 5) as nat == ((upto - 1) * 5) as nat);
        digit_from_bit_bounded(bytes, ((upto - 1) * 5) as nat);
        decode_char_of_alphabet_digit(digit_from_bit_spec(bytes, ((upto - 1) * 5) as nat));
    }
}

proof fn reconstruct_prefix_len_lower(bytes: Seq<u8>, upto: nat)
    ensures
        upto == 0 ==> reconstruct_prefix_spec(bytes, upto).len() == 0,
        upto != 0 ==> reconstruct_prefix_spec(bytes, upto).len() >= (((upto - 1) * 5) / 8) + 1,
    decreases upto
{
    if upto != 0 {
        reconstruct_prefix_len_lower(bytes, (upto - 1) as nat);
        let n = (upto - 1) as nat;
        let bit = n * 5;
        let i = bit / 8;
        let digit = digit_from_bit_spec(bytes, bit);
        let low = decode_low_spec(bit, digit);
        or_extend_at_len(reconstruct_prefix_spec(bytes, n), i, low);
        assert(reconstruct_prefix_spec(bytes, upto).len() >= i + 1);
        assert(i == (((upto - 1) * 5) / 8));
    }
}

proof fn reconstruct_prefix_len_upper(bytes: Seq<u8>, upto: nat)
    requires upto <= encoded_length_spec(bytes.len())
    ensures reconstruct_prefix_spec(bytes, upto).len() <= bytes.len()
    decreases upto
{
    if upto != 0 {
        reconstruct_prefix_len_upper(bytes, (upto - 1) as nat);
        let n = (upto - 1) as nat;
        let bit = n * 5;
        let i = bit / 8;
        let digit = digit_from_bit_spec(bytes, bit);
        let low = decode_low_spec(bit, digit);
        let high = decode_high_spec(bit, digit);
        let acc = reconstruct_prefix_spec(bytes, n);
        chunk_start_in_bounds(bytes.len(), n);
        or_extend_at_len(acc, i, low);
        assert(or_extend_at_spec(acc, i, low).len() <= bytes.len()) by {
            if acc.len() <= i {
                assert(or_extend_at_spec(acc, i, low).len() == i + 1);
                assert(i + 1 <= bytes.len());
            } else {
                assert(or_extend_at_spec(acc, i, low).len() == acc.len());
            }
        }
        if high == 0u8 {
            assert(reconstruct_prefix_spec(bytes, upto).len() == or_extend_at_spec(acc, i, low).len());
        } else {
            digit_from_bit_bounded(bytes, bit);
            if bit % 8 <= 3 {
                decode_high_zero_when_no_spill(bit, digit);
                assert(high == 0u8);
            } else if i + 1 == bytes.len() {
                last_chunk_spill_zero(bytes, n);
                assert(high == 0u8);
            } else {
                assert(i + 2 <= bytes.len());
                or_extend_at_len(or_extend_at_spec(acc, i, low), i + 1, high);
                assert(reconstruct_prefix_spec(bytes, upto).len() <= bytes.len()) by {
                    assert(reconstruct_prefix_spec(bytes, upto) == or_extend_at_spec(or_extend_at_spec(acc, i, low), i + 1, high));
                    if or_extend_at_spec(acc, i, low).len() <= i + 1 {
                        assert(reconstruct_prefix_spec(bytes, upto).len() == i + 2);
                    } else {
                        assert(reconstruct_prefix_spec(bytes, upto).len() == or_extend_at_spec(acc, i, low).len());
                    }
                }
            }
        }
    }
}

proof fn reconstruct_full_len(bytes: Seq<u8>)
    ensures reconstruct_prefix_spec(bytes, encoded_length_spec(bytes.len())).len() == bytes.len()
{
    let out_len = encoded_length_spec(bytes.len());
    reconstruct_prefix_len_lower(bytes, out_len);
    reconstruct_prefix_len_upper(bytes, out_len);
    encoded_length_ceil(bytes.len());
    if out_len == 0 {
        assert(bytes.len() == 0);
    } else {
        assert((((out_len - 1) * 5) / 8) + 1 == bytes.len()) by(nonlinear_arith)
            requires
                out_len == encoded_length_spec(bytes.len()),
                (out_len - 1) * 5 < bytes.len() * 8,
                out_len * 5 >= bytes.len() * 8,
                bytes.len() != 0;
    }
}

pub closed spec fn low_mask_spec(bits: nat) -> u8
    recommends bits <= 8
{
    if bits == 0 { 0u8 }
    else if bits == 1 { 1u8 }
    else if bits == 2 { 3u8 }
    else if bits == 3 { 7u8 }
    else if bits == 4 { 15u8 }
    else if bits == 5 { 31u8 }
    else if bits == 6 { 63u8 }
    else if bits == 7 { 127u8 }
    else { 255u8 }
}

pub closed spec fn written_bits_in_byte_spec(upto: nat, idx: nat) -> nat {
    let boundary = upto * 5;
    let start = idx * 8;
    if boundary <= start {
        0
    } else if boundary >= start + 8 {
        8
    } else {
        (boundary - start) as nat
    }
}

pub closed spec fn prefix_masked_byte_spec(bytes: Seq<u8>, upto: nat, idx: nat) -> u8
    recommends idx < bytes.len()
{
    (((bytes[idx as int] as u64) & (low_mask_spec(written_bits_in_byte_spec(upto, idx)) as u64)) as u8)
}

pub closed spec fn prefix_masked_bytes_spec(bytes: Seq<u8>, upto: nat) -> Seq<u8> {
    Seq::new(bytes.len(), |i: int| prefix_masked_byte_spec(bytes, upto, i as nat))
}

proof fn prefix_masked_full(bytes: Seq<u8>)
    ensures prefix_masked_bytes_spec(bytes, encoded_length_spec(bytes.len())) == bytes
{
    let out_len = encoded_length_spec(bytes.len());
    encoded_length_ceil(bytes.len());
    assert forall|i: int| 0 <= i < bytes.len() implies #[trigger] prefix_masked_bytes_spec(bytes, out_len)[i] == bytes[i] by {
        let idx = i as nat;
        assert(out_len * 5 >= bytes.len() * 8);
        assert(out_len * 5 >= idx * 8 + 8) by(nonlinear_arith)
            requires out_len * 5 >= bytes.len() * 8, idx < bytes.len();
        assert(written_bits_in_byte_spec(out_len, idx) == 8);
        assert(low_mask_spec(written_bits_in_byte_spec(out_len, idx)) == 255u8);
        let b = bytes[i];
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
    }
}

proof fn extend_to_index(acc: Seq<u8>, len: nat, idx: nat)
    requires
        acc.len() <= len,
        idx < len,
    ensures
        idx < acc.len() ==> extend_to_spec(acc, len)[idx as int] == acc[idx as int],
        acc.len() <= idx ==> extend_to_spec(acc, len)[idx as int] == 0u8,
{}

proof fn or_extend_at_full_other(acc: Seq<u8>, len: nat, idx: nat, val: u8, k: nat)
    requires
        acc.len() <= len,
        idx < len,
        k < len,
        k != idx,
    ensures
        extend_to_spec(or_extend_at_spec(acc, idx, val), len)[k as int]
            == extend_to_spec(acc, len)[k as int],
{
    or_extend_at_len(acc, idx, val);
    if acc.len() <= idx {
        assert(or_extend_at_spec(acc, idx, val)
            == extend_to_spec(acc, idx + 1).update(
                idx as int,
                (((extend_to_spec(acc, idx + 1)[idx as int] as u64) | (val as u64)) as u8),
            ));
        if k < idx {
            extend_to_index(or_extend_at_spec(acc, idx, val), len, k);
            extend_to_index(acc, len, k);
            extend_to_index(acc, idx + 1, k);
            assert(or_extend_at_spec(acc, idx, val)[k as int] == extend_to_spec(acc, idx + 1)[k as int]);
            if k < acc.len() {
                assert(extend_to_spec(acc, len)[k as int] == acc[k as int]);
                assert(extend_to_spec(acc, idx + 1)[k as int] == acc[k as int]);
            } else {
                assert(extend_to_spec(acc, len)[k as int] == 0u8);
                assert(extend_to_spec(acc, idx + 1)[k as int] == 0u8);
            }
        } else {
            assert(idx < k) by(nonlinear_arith)
                requires k != idx, !(k < idx);
            extend_to_index(or_extend_at_spec(acc, idx, val), len, k);
            extend_to_index(acc, len, k);
            assert(acc.len() <= k) by(nonlinear_arith)
                requires acc.len() <= idx, idx < k;
        }
    } else {
        assert(or_extend_at_spec(acc, idx, val)
            == acc.update(idx as int, (((acc[idx as int] as u64) | (val as u64)) as u8)));
        if k < acc.len() {
            extend_to_index(or_extend_at_spec(acc, idx, val), len, k);
            extend_to_index(acc, len, k);
            assert(or_extend_at_spec(acc, idx, val)[k as int] == acc[k as int]);
        } else {
            extend_to_index(or_extend_at_spec(acc, idx, val), len, k);
            extend_to_index(acc, len, k);
        }
    }
}

proof fn or_extend_at_full_index(acc: Seq<u8>, len: nat, idx: nat, val: u8)
    requires
        acc.len() <= len,
        idx < len,
    ensures
        extend_to_spec(or_extend_at_spec(acc, idx, val), len)[idx as int]
            == (((extend_to_spec(acc, len)[idx as int] as u64) | (val as u64)) as u8),
{
    or_extend_at_len(acc, idx, val);
    if acc.len() <= idx {
        assert(or_extend_at_spec(acc, idx, val)
            == extend_to_spec(acc, idx + 1).update(
                idx as int,
                (((extend_to_spec(acc, idx + 1)[idx as int] as u64) | (val as u64)) as u8),
            ));
        extend_to_index(or_extend_at_spec(acc, idx, val), len, idx);
        extend_to_index(acc, len, idx);
        extend_to_index(acc, idx + 1, idx);
        assert(or_extend_at_spec(acc, idx, val)[idx as int]
            == (((extend_to_spec(acc, idx + 1)[idx as int] as u64) | (val as u64)) as u8));
    } else {
        assert(or_extend_at_spec(acc, idx, val)
            == acc.update(idx as int, (((acc[idx as int] as u64) | (val as u64)) as u8)));
        extend_to_index(or_extend_at_spec(acc, idx, val), len, idx);
        extend_to_index(acc, len, idx);
        assert(or_extend_at_spec(acc, idx, val)[idx as int]
            == (((acc[idx as int] as u64) | (val as u64)) as u8));
    }
}

proof fn prefix_masked_zero(bytes: Seq<u8>)
    ensures prefix_masked_bytes_spec(bytes, 0) == zero_bytes_spec(bytes.len())
{
    assert forall|i: int| 0 <= i < bytes.len() implies #[trigger] prefix_masked_bytes_spec(bytes, 0)[i] == 0u8 by {
        assert(written_bits_in_byte_spec(0, i as nat) == 0);
        assert(low_mask_spec(written_bits_in_byte_spec(0, i as nat)) == 0u8);
        let b = bytes[i];
        assert((((b as u64) & 0u64) as u8) == 0u8) by(bit_vector);
    }
}

proof fn current_byte_mask_update_no_spill(byte: u8, offset: u64)
    requires offset <= 3
    ensures
        (((((byte as u64) & (sub(1u64 << offset, 1u64)))
            | (((((byte as u64) >> offset) & 31u64) << offset) & 255u64)) as u8))
            == ((byte as u64) & sub(1u64 << (offset + 5u64), 1u64)) as u8
{
    assert(
        ((((byte as u64) & sub(1u64 << offset, 1u64))
            | (((((byte as u64) >> offset) & 31u64) << offset) & 255u64)) as u8)
            == ((byte as u64) & sub(1u64 << (offset + 5u64), 1u64)) as u8
    ) by(bit_vector)
        requires offset <= 3u64;
}

proof fn current_byte_mask_update_spill(byte: u8, offset: u64)
    requires 4 <= offset <= 7
    ensures
        (((((byte as u64) & (sub(1u64 << offset, 1u64)))
            | (((((byte as u64) >> offset) & 31u64) << offset) & 255u64)) as u8))
            == byte
{
    assert(
        ((((byte as u64) & sub(1u64 << offset, 1u64))
            | (((((byte as u64) >> offset) & 31u64) << offset) & 255u64)) as u8)
            == byte
    ) by(bit_vector)
        requires 4u64 <= offset <= 7u64;
}

proof fn same_byte_digit_formula_j0(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 0,
    ensures
        digit_from_bit_spec(bytes, n * 5) == (((bytes[((n * 5) / 8) as int] as u64) & 31u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    assert(bit % 8 == 0);
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 0u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 1u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 2u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((b as u64) >> 3u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((b as u64) >> 4u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == (((b as u64) & 31u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 0u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 1u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 2u64) & 1u64) as u8),
            b3 == ((((b as u64) >> 3u64) & 1u64) as u8),
            b4 == ((((b as u64) >> 4u64) & 1u64) as u8);
}

proof fn same_byte_digit_formula_j1(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 1,
    ensures
        digit_from_bit_spec(bytes, n * 5) == ((((bytes[((n * 5) / 8) as int] as u64) >> 1u64) & 31u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    assert(bit % 8 == 1);
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 1u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 2u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 3u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((b as u64) >> 4u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((b as u64) >> 5u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 1u64) & 31u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 1u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 2u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 3u64) & 1u64) as u8),
            b3 == ((((b as u64) >> 4u64) & 1u64) as u8),
            b4 == ((((b as u64) >> 5u64) & 1u64) as u8);
}

proof fn same_byte_digit_formula_j2(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 2,
    ensures
        digit_from_bit_spec(bytes, n * 5) == ((((bytes[((n * 5) / 8) as int] as u64) >> 2u64) & 31u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    assert(bit % 8 == 2);
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 2u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 3u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 4u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((b as u64) >> 5u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((b as u64) >> 6u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 2u64) & 31u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 2u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 3u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 4u64) & 1u64) as u8),
            b3 == ((((b as u64) >> 5u64) & 1u64) as u8),
            b4 == ((((b as u64) >> 6u64) & 1u64) as u8);
}

proof fn same_byte_digit_formula_j3(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 3,
    ensures
        digit_from_bit_spec(bytes, n * 5) == ((((bytes[((n * 5) / 8) as int] as u64) >> 3u64) & 31u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    assert(bit % 8 == 3);
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 3u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 4u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 5u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((b as u64) >> 7u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 3u64) & 31u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 3u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 4u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 5u64) & 1u64) as u8),
            b3 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b4 == ((((b as u64) >> 7u64) & 1u64) as u8);
}

proof fn spill_digit_formula_j4(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 4,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == (((((bytes[((n * 5) / 8) as int] as u64) >> 4u64) & 15u64)
                | (((bytes[(((n * 5) / 8) + 1) as int] as u64) & 1u64) << 4u64)) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let c = bytes[(i + 1) as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 4) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 4;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 4u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 5u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert((bit + 4) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 4;
    assert((bit + 4) % 8 == 0) by(nonlinear_arith)
        requires bit == 8 * i + 4;
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((c as u64) >> 0u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == (((((b as u64) >> 4u64) & 15u64) | (((c as u64) & 1u64) << 4u64)) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 4u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 5u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b3 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b4 == ((((c as u64) >> 0u64) & 1u64) as u8);
}

proof fn spill_digit_formula_j5(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 5,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == (((((bytes[((n * 5) / 8) as int] as u64) >> 5u64) & 7u64)
                | (((bytes[(((n * 5) / 8) + 1) as int] as u64) & 3u64) << 3u64)) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let c = bytes[(i + 1) as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 5) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 5;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 5u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert((bit + 3) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 5;
    assert((bit + 3) % 8 == 0) by(nonlinear_arith)
        requires bit == 8 * i + 5;
    assert((bit + 4) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 5;
    assert((bit + 4) % 8 == 1) by(nonlinear_arith)
        requires bit == 8 * i + 5;
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((c as u64) >> 0u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((c as u64) >> 1u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == (((((b as u64) >> 5u64) & 7u64) | (((c as u64) & 3u64) << 3u64)) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 5u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b3 == ((((c as u64) >> 0u64) & 1u64) as u8),
            b4 == ((((c as u64) >> 1u64) & 1u64) as u8);
}

proof fn spill_digit_formula_j6(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 6,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == (((((bytes[((n * 5) / 8) as int] as u64) >> 6u64) & 3u64)
                | (((bytes[(((n * 5) / 8) + 1) as int] as u64) & 7u64) << 2u64)) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let c = bytes[(i + 1) as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 6) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 6;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert((bit + 2) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 6;
    assert((bit + 2) % 8 == 0) by(nonlinear_arith)
        requires bit == 8 * i + 6;
    assert((bit + 3) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 6;
    assert((bit + 3) % 8 == 1) by(nonlinear_arith)
        requires bit == 8 * i + 6;
    assert((bit + 4) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 6;
    assert((bit + 4) % 8 == 2) by(nonlinear_arith)
        requires bit == 8 * i + 6;
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((c as u64) >> 0u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((c as u64) >> 1u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((c as u64) >> 2u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == (((((b as u64) >> 6u64) & 3u64) | (((c as u64) & 7u64) << 2u64)) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b2 == ((((c as u64) >> 0u64) & 1u64) as u8),
            b3 == ((((c as u64) >> 1u64) & 1u64) as u8),
            b4 == ((((c as u64) >> 2u64) & 1u64) as u8);
}

proof fn spill_digit_formula_j7(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 7,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == (((((bytes[((n * 5) / 8) as int] as u64) >> 7u64) & 1u64)
                | (((bytes[(((n * 5) / 8) + 1) as int] as u64) & 15u64) << 1u64)) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let c = bytes[(i + 1) as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 7) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 7;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert((bit + 1) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 1) % 8 == 0) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 2) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 2) % 8 == 1) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 3) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 3) % 8 == 2) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 4) / 8 == i + 1) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert((bit + 4) % 8 == 3) by(nonlinear_arith)
        requires bit == 8 * i + 7;
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((c as u64) >> 0u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((c as u64) >> 1u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((c as u64) >> 2u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 4) == ((((c as u64) >> 3u64) & 1u64) as u8));
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == (((((b as u64) >> 7u64) & 1u64) | (((c as u64) & 15u64) << 1u64)) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b1 == ((((c as u64) >> 0u64) & 1u64) as u8),
            b2 == ((((c as u64) >> 1u64) & 1u64) as u8),
            b3 == ((((c as u64) >> 2u64) & 1u64) as u8),
            b4 == ((((c as u64) >> 3u64) & 1u64) as u8);
}

proof fn last_spill_digit_formula_j4(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 4,
        ((n * 5) / 8) + 1 == bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == ((((bytes[((n * 5) / 8) as int] as u64) >> 4u64) & 15u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 4) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 4;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 4u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 5u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 3) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert(bit + 4 == bytes.len() * 8) by(nonlinear_arith)
        requires bit == 8 * i + 4, i + 1 == bytes.len();
    bit_at_padded_zero_beyond_end(bytes, bit + 4);
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(b4 == 0u8);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 4u64) & 15u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 4u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 5u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b3 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b4 == 0u8;
}

proof fn last_spill_digit_formula_j5(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 5,
        ((n * 5) / 8) + 1 == bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == ((((bytes[((n * 5) / 8) as int] as u64) >> 5u64) & 7u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 5) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 5;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 5u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 2) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert(bit + 3 == bytes.len() * 8) by(nonlinear_arith)
        requires bit == 8 * i + 5, i + 1 == bytes.len();
    bit_at_padded_zero_beyond_end(bytes, bit + 3);
    bit_at_padded_zero_beyond_end(bytes, bit + 4);
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(b3 == 0u8);
    assert(b4 == 0u8);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 5u64) & 7u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 5u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b2 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b3 == 0u8,
            b4 == 0u8;
}

proof fn last_spill_digit_formula_j6(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 6,
        ((n * 5) / 8) + 1 == bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == ((((bytes[((n * 5) / 8) as int] as u64) >> 6u64) & 3u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 6) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 6;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 6u64) & 1u64) as u8));
    assert(bit_at_padded_spec(bytes, bit + 1) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert(bit + 2 == bytes.len() * 8) by(nonlinear_arith)
        requires bit == 8 * i + 6, i + 1 == bytes.len();
    bit_at_padded_zero_beyond_end(bytes, bit + 2);
    bit_at_padded_zero_beyond_end(bytes, bit + 3);
    bit_at_padded_zero_beyond_end(bytes, bit + 4);
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(b2 == 0u8);
    assert(b3 == 0u8);
    assert(b4 == 0u8);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 6u64) & 3u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 6u64) & 1u64) as u8),
            b1 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b2 == 0u8,
            b3 == 0u8,
            b4 == 0u8;
}

proof fn last_spill_digit_formula_j7(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 == 7,
        ((n * 5) / 8) + 1 == bytes.len(),
    ensures
        digit_from_bit_spec(bytes, n * 5)
            == ((((bytes[((n * 5) / 8) as int] as u64) >> 7u64) & 1u64) as u8)
{
    let bit = n * 5;
    let i = bit / 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    chunk_start_in_bounds(bytes.len(), n);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + 7) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            bit % 8 == 7;
    assert(bit_at_padded_spec(bytes, bit) == ((((b as u64) >> 7u64) & 1u64) as u8));
    assert(bit + 1 == bytes.len() * 8) by(nonlinear_arith)
        requires bit == 8 * i + 7, i + 1 == bytes.len();
    bit_at_padded_zero_beyond_end(bytes, bit + 1);
    bit_at_padded_zero_beyond_end(bytes, bit + 2);
    bit_at_padded_zero_beyond_end(bytes, bit + 3);
    bit_at_padded_zero_beyond_end(bytes, bit + 4);
    let b0 = bit_at_padded_spec(bytes, bit);
    let b1 = bit_at_padded_spec(bytes, bit + 1);
    let b2 = bit_at_padded_spec(bytes, bit + 2);
    let b3 = bit_at_padded_spec(bytes, bit + 3);
    let b4 = bit_at_padded_spec(bytes, bit + 4);
    assert(b1 == 0u8);
    assert(b2 == 0u8);
    assert(b3 == 0u8);
    assert(b4 == 0u8);
    assert(d == ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8);
    assert(d == ((((b as u64) >> 7u64) & 1u64) as u8)) by(bit_vector)
        requires
            d == ((b0 as u64)
                | ((b1 as u64) << 1u64)
                | ((b2 as u64) << 2u64)
                | ((b3 as u64) << 3u64)
                | ((b4 as u64) << 4u64)) as u8,
            b0 == ((((b as u64) >> 7u64) & 1u64) as u8),
            b1 == 0u8,
            b2 == 0u8,
            b3 == 0u8,
            b4 == 0u8;
}

proof fn current_byte_update_last_spill(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        4 <= (n * 5) % 8 <= 7,
        ((n * 5) / 8) + 1 == bytes.len(),
    ensures
        (((prefix_masked_byte_spec(bytes, n, (n * 5) / 8) as u64)
            | (decode_low_spec(n * 5, digit_from_bit_spec(bytes, n * 5)) as u64)) as u8)
            == prefix_masked_byte_spec(bytes, n + 1, (n * 5) / 8)
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let start = i * 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    div_mod_decompose_8(bit);
    assert(bit == start + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8,
            start == i * 8;
    if j == 4 {
        last_spill_digit_formula_j4(bytes, n);
        assert(bit == start + 4) by(nonlinear_arith)
            requires bit == start + j, j == 4;
        assert(written_bits_in_byte_spec(n, i) == 4) by {
            assert(n * 5 == start + 4) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 4;
        }
        assert((n + 1) * 5 == start + 9) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 4;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 9) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 4;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 15u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 15u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 4u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 15u64) as u8),
                after == b,
                low == ((((d as u64) << 4u64) & 255u64) as u8),
                d == ((((b as u64) >> 4u64) & 15u64) as u8);
    } else if j == 5 {
        last_spill_digit_formula_j5(bytes, n);
        assert(bit == start + 5) by(nonlinear_arith)
            requires bit == start + j, j == 5;
        assert(written_bits_in_byte_spec(n, i) == 5) by {
            assert(n * 5 == start + 5) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 5;
        }
        assert((n + 1) * 5 == start + 10) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 5;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 10) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 5;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 31u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 31u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 5u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 31u64) as u8),
                after == b,
                low == ((((d as u64) << 5u64) & 255u64) as u8),
                d == ((((b as u64) >> 5u64) & 7u64) as u8);
    } else if j == 6 {
        last_spill_digit_formula_j6(bytes, n);
        assert(bit == start + 6) by(nonlinear_arith)
            requires bit == start + j, j == 6;
        assert(written_bits_in_byte_spec(n, i) == 6) by {
            assert(n * 5 == start + 6) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 6;
        }
        assert((n + 1) * 5 == start + 11) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 6;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 11) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 6;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 63u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 63u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 6u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 63u64) as u8),
                after == b,
                low == ((((d as u64) << 6u64) & 255u64) as u8),
                d == ((((b as u64) >> 6u64) & 3u64) as u8);
    } else {
        assert(j == 7);
        last_spill_digit_formula_j7(bytes, n);
        assert(bit == start + 7) by(nonlinear_arith)
            requires bit == start + j, j == 7;
        assert(written_bits_in_byte_spec(n, i) == 7) by {
            assert(n * 5 == start + 7) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 7;
        }
        assert((n + 1) * 5 == start + 12) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 7;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 12) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 7;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 127u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 127u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 7u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 127u64) as u8),
                after == b,
                low == ((((d as u64) << 7u64) & 255u64) as u8),
                d == ((((b as u64) >> 7u64) & 1u64) as u8);
    }
}

proof fn current_byte_update_no_spill(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 <= 3,
    ensures
        (((prefix_masked_byte_spec(bytes, n, (n * 5) / 8) as u64)
            | (decode_low_spec(n * 5, digit_from_bit_spec(bytes, n * 5)) as u64)) as u8)
            == prefix_masked_byte_spec(bytes, n + 1, (n * 5) / 8)
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let start = i * 8;
    let b = bytes[i as int];
    let d = digit_from_bit_spec(bytes, bit);
    div_mod_decompose_8(bit);
    assert(bit == start + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8,
            start == i * 8;
    if j == 0 {
        same_byte_digit_formula_j0(bytes, n);
        assert(bit == start) by(nonlinear_arith)
            requires bit == start + j, j == 0;
        assert(written_bits_in_byte_spec(n, i) == 0) by {
            assert(n * 5 == start) by(nonlinear_arith)
                requires bit == n * 5, bit == start;
        }
        assert((n + 1) * 5 == start + 5) by(nonlinear_arith)
            requires bit == n * 5, bit == start;
        assert(written_bits_in_byte_spec(n + 1, i) == 5) by {
            assert((n + 1) * 5 == start + 5) by(nonlinear_arith)
                requires bit == n * 5, bit == start;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 0u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 31u8);
        assert(before == (((b as u64) & 0u64) as u8));
        assert(after == (((b as u64) & 31u64) as u8));
        assert((((b as u64) & 0u64) as u8) == 0u8) by(bit_vector);
        assert(before == 0u8);
        assert(low == ((((d as u64) << 0u64) & 255u64) as u8));
        assert(((((d as u64) << 0u64) & 255u64) as u8) == d) by(bit_vector);
        assert(low == d);
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == 0u8,
                after == (((b as u64) & 31u64) as u8),
                low == d,
                d == (((b as u64) & 31u64) as u8);
    } else if j == 1 {
        same_byte_digit_formula_j1(bytes, n);
        assert(bit == start + 1) by(nonlinear_arith)
            requires bit == start + j, j == 1;
        assert(written_bits_in_byte_spec(n, i) == 1) by {
            assert(n * 5 == start + 1) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 1;
        }
        assert((n + 1) * 5 == start + 6) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 1;
        assert(written_bits_in_byte_spec(n + 1, i) == 6) by {
            assert((n + 1) * 5 == start + 6) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 1;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 1u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 63u8);
        assert(before == (((b as u64) & 1u64) as u8));
        assert(after == (((b as u64) & 63u64) as u8));
        assert(low == ((((d as u64) << 1u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 1u64) as u8),
                after == (((b as u64) & 63u64) as u8),
                low == ((((d as u64) << 1u64) & 255u64) as u8),
                d == ((((b as u64) >> 1u64) & 31u64) as u8);
    } else if j == 2 {
        same_byte_digit_formula_j2(bytes, n);
        assert(bit == start + 2) by(nonlinear_arith)
            requires bit == start + j, j == 2;
        assert(written_bits_in_byte_spec(n, i) == 2) by {
            assert(n * 5 == start + 2) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 2;
        }
        assert((n + 1) * 5 == start + 7) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 2;
        assert(written_bits_in_byte_spec(n + 1, i) == 7) by {
            assert((n + 1) * 5 == start + 7) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 2;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 3u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 127u8);
        assert(before == (((b as u64) & 3u64) as u8));
        assert(after == (((b as u64) & 127u64) as u8));
        assert(low == ((((d as u64) << 2u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 3u64) as u8),
                after == (((b as u64) & 127u64) as u8),
                low == ((((d as u64) << 2u64) & 255u64) as u8),
                d == ((((b as u64) >> 2u64) & 31u64) as u8);
    } else {
        assert(j == 3);
        same_byte_digit_formula_j3(bytes, n);
        assert(bit == start + 3) by(nonlinear_arith)
            requires bit == start + j, j == 3;
        assert(written_bits_in_byte_spec(n, i) == 3) by {
            assert(n * 5 == start + 3) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 3;
        }
        assert((n + 1) * 5 == start + 8) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 3;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 8) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 3;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 7u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 7u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 3u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 7u64) as u8),
                after == b,
                low == ((((d as u64) << 3u64) & 255u64) as u8),
                d == ((((b as u64) >> 3u64) & 31u64) as u8);
    }
}

proof fn current_byte_update_spill(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        4 <= (n * 5) % 8 <= 7,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        (((prefix_masked_byte_spec(bytes, n, (n * 5) / 8) as u64)
            | (decode_low_spec(n * 5, digit_from_bit_spec(bytes, n * 5)) as u64)) as u8)
            == prefix_masked_byte_spec(bytes, n + 1, (n * 5) / 8)
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let start = i * 8;
    let b = bytes[i as int];
    let c = bytes[(i + 1) as int];
    let d = digit_from_bit_spec(bytes, bit);
    div_mod_decompose_8(bit);
    assert(bit == start + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8,
            start == i * 8;
    if j == 4 {
        spill_digit_formula_j4(bytes, n);
        assert(bit == start + 4) by(nonlinear_arith)
            requires bit == start + j, j == 4;
        assert(written_bits_in_byte_spec(n, i) == 4) by {
            assert(n * 5 == start + 4) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 4;
        }
        assert((n + 1) * 5 == start + 9) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 4;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 9) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 4;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 15u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 15u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 4u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 15u64) as u8),
                after == b,
                low == ((((d as u64) << 4u64) & 255u64) as u8),
                d == (((((b as u64) >> 4u64) & 15u64) | (((c as u64) & 1u64) << 4u64)) as u8);
    } else if j == 5 {
        spill_digit_formula_j5(bytes, n);
        assert(bit == start + 5) by(nonlinear_arith)
            requires bit == start + j, j == 5;
        assert(written_bits_in_byte_spec(n, i) == 5) by {
            assert(n * 5 == start + 5) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 5;
        }
        assert((n + 1) * 5 == start + 10) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 5;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 10) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 5;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 31u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 31u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 5u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 31u64) as u8),
                after == b,
                low == ((((d as u64) << 5u64) & 255u64) as u8),
                d == (((((b as u64) >> 5u64) & 7u64) | (((c as u64) & 3u64) << 3u64)) as u8);
    } else if j == 6 {
        spill_digit_formula_j6(bytes, n);
        assert(bit == start + 6) by(nonlinear_arith)
            requires bit == start + j, j == 6;
        assert(written_bits_in_byte_spec(n, i) == 6) by {
            assert(n * 5 == start + 6) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 6;
        }
        assert((n + 1) * 5 == start + 11) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 6;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 11) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 6;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 63u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 63u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 6u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 63u64) as u8),
                after == b,
                low == ((((d as u64) << 6u64) & 255u64) as u8),
                d == (((((b as u64) >> 6u64) & 3u64) | (((c as u64) & 7u64) << 2u64)) as u8);
    } else {
        assert(j == 7);
        spill_digit_formula_j7(bytes, n);
        assert(bit == start + 7) by(nonlinear_arith)
            requires bit == start + j, j == 7;
        assert(written_bits_in_byte_spec(n, i) == 7) by {
            assert(n * 5 == start + 7) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 7;
        }
        assert((n + 1) * 5 == start + 12) by(nonlinear_arith)
            requires bit == n * 5, bit == start + 7;
        assert(written_bits_in_byte_spec(n + 1, i) == 8) by {
            assert((n + 1) * 5 == start + 12) by(nonlinear_arith)
                requires bit == n * 5, bit == start + 7;
        }
        let before = prefix_masked_byte_spec(bytes, n, i);
        let after = prefix_masked_byte_spec(bytes, n + 1, i);
        let low = decode_low_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, i)) == 127u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, i)) == 255u8);
        assert(before == (((b as u64) & 127u64) as u8));
        assert(after == (((b as u64) & 255u64) as u8));
        assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
        assert(after == b);
        assert(low == ((((d as u64) << 7u64) & 255u64) as u8));
        assert((((before as u64) | (low as u64)) as u8) == after) by(bit_vector)
            requires
                before == (((b as u64) & 127u64) as u8),
                after == b,
                low == ((((d as u64) << 7u64) & 255u64) as u8),
                d == (((((b as u64) >> 7u64) & 1u64) | (((c as u64) & 15u64) << 1u64)) as u8);
    }
}

proof fn next_byte_update_spill(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        4 <= (n * 5) % 8 <= 7,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        (((prefix_masked_byte_spec(bytes, n, ((n * 5) / 8) + 1) as u64)
            | (decode_high_spec(n * 5, digit_from_bit_spec(bytes, n * 5)) as u64)) as u8)
            == prefix_masked_byte_spec(bytes, n + 1, ((n * 5) / 8) + 1)
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let next = i + 1;
    let start = next * 8;
    let b = bytes[i as int];
    let c = bytes[next as int];
    let d = digit_from_bit_spec(bytes, bit);
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8;
    assert(n * 5 < start) by(nonlinear_arith)
        requires
            bit == n * 5,
            bit == 8 * i + j,
            next == i + 1,
            start == next * 8,
            j <= 7;
    assert(written_bits_in_byte_spec(n, next) == 0);
    if j == 4 {
        spill_digit_formula_j4(bytes, n);
        assert((n + 1) * 5 == start + 1) by(nonlinear_arith)
            requires bit == n * 5, bit == 8 * i + 4, next == i + 1, start == next * 8;
        assert(written_bits_in_byte_spec(n + 1, next) == 1) by {
            assert((n + 1) * 5 == start + 1) by(nonlinear_arith)
                requires bit == n * 5, bit == 8 * i + 4, next == i + 1, start == next * 8;
        }
        let before = prefix_masked_byte_spec(bytes, n, next);
        let after = prefix_masked_byte_spec(bytes, n + 1, next);
        let high = decode_high_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, next)) == 0u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, next)) == 1u8);
        assert(before == (((c as u64) & 0u64) as u8));
        assert((((c as u64) & 0u64) as u8) == 0u8) by(bit_vector);
        assert(before == 0u8);
        assert(after == (((c as u64) & 1u64) as u8));
        assert(high == (((d as u64) >> 4u64) as u8));
        assert((((before as u64) | (high as u64)) as u8) == after) by(bit_vector)
            requires
                before == 0u8,
                after == (((c as u64) & 1u64) as u8),
                high == (((d as u64) >> 4u64) as u8),
                d == (((((b as u64) >> 4u64) & 15u64)
                    | (((c as u64) & 1u64) << 4u64)) as u8);
    } else if j == 5 {
        spill_digit_formula_j5(bytes, n);
        assert((n + 1) * 5 == start + 2) by(nonlinear_arith)
            requires bit == n * 5, bit == 8 * i + 5, next == i + 1, start == next * 8;
        assert(written_bits_in_byte_spec(n + 1, next) == 2) by {
            assert((n + 1) * 5 == start + 2) by(nonlinear_arith)
                requires bit == n * 5, bit == 8 * i + 5, next == i + 1, start == next * 8;
        }
        let before = prefix_masked_byte_spec(bytes, n, next);
        let after = prefix_masked_byte_spec(bytes, n + 1, next);
        let high = decode_high_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, next)) == 0u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, next)) == 3u8);
        assert(before == (((c as u64) & 0u64) as u8));
        assert((((c as u64) & 0u64) as u8) == 0u8) by(bit_vector);
        assert(before == 0u8);
        assert(after == (((c as u64) & 3u64) as u8));
        assert(high == (((d as u64) >> 3u64) as u8));
        assert((((before as u64) | (high as u64)) as u8) == after) by(bit_vector)
            requires
                before == 0u8,
                after == (((c as u64) & 3u64) as u8),
                high == (((d as u64) >> 3u64) as u8),
                d == (((((b as u64) >> 5u64) & 7u64)
                    | (((c as u64) & 3u64) << 3u64)) as u8);
    } else if j == 6 {
        spill_digit_formula_j6(bytes, n);
        assert((n + 1) * 5 == start + 3) by(nonlinear_arith)
            requires bit == n * 5, bit == 8 * i + 6, next == i + 1, start == next * 8;
        assert(written_bits_in_byte_spec(n + 1, next) == 3) by {
            assert((n + 1) * 5 == start + 3) by(nonlinear_arith)
                requires bit == n * 5, bit == 8 * i + 6, next == i + 1, start == next * 8;
        }
        let before = prefix_masked_byte_spec(bytes, n, next);
        let after = prefix_masked_byte_spec(bytes, n + 1, next);
        let high = decode_high_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, next)) == 0u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, next)) == 7u8);
        assert(before == (((c as u64) & 0u64) as u8));
        assert((((c as u64) & 0u64) as u8) == 0u8) by(bit_vector);
        assert(before == 0u8);
        assert(after == (((c as u64) & 7u64) as u8));
        assert(high == (((d as u64) >> 2u64) as u8));
        assert((((before as u64) | (high as u64)) as u8) == after) by(bit_vector)
            requires
                before == 0u8,
                after == (((c as u64) & 7u64) as u8),
                high == (((d as u64) >> 2u64) as u8),
                d == (((((b as u64) >> 6u64) & 3u64)
                    | (((c as u64) & 7u64) << 2u64)) as u8);
    } else {
        assert(j == 7);
        spill_digit_formula_j7(bytes, n);
        assert((n + 1) * 5 == start + 4) by(nonlinear_arith)
            requires bit == n * 5, bit == 8 * i + 7, next == i + 1, start == next * 8;
        assert(written_bits_in_byte_spec(n + 1, next) == 4) by {
            assert((n + 1) * 5 == start + 4) by(nonlinear_arith)
                requires bit == n * 5, bit == 8 * i + 7, next == i + 1, start == next * 8;
        }
        let before = prefix_masked_byte_spec(bytes, n, next);
        let after = prefix_masked_byte_spec(bytes, n + 1, next);
        let high = decode_high_spec(bit, d);
        assert(low_mask_spec(written_bits_in_byte_spec(n, next)) == 0u8);
        assert(low_mask_spec(written_bits_in_byte_spec(n + 1, next)) == 15u8);
        assert(before == (((c as u64) & 0u64) as u8));
        assert((((c as u64) & 0u64) as u8) == 0u8) by(bit_vector);
        assert(before == 0u8);
        assert(after == (((c as u64) & 15u64) as u8));
        assert(high == (((d as u64) >> 1u64) as u8));
        assert((((before as u64) | (high as u64)) as u8) == after) by(bit_vector)
            requires
                before == 0u8,
                after == (((c as u64) & 15u64) as u8),
                high == (((d as u64) >> 1u64) as u8),
                d == (((((b as u64) >> 7u64) & 1u64)
                    | (((c as u64) & 15u64) << 1u64)) as u8);
    }
}

proof fn next_byte_zero_before_step(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        prefix_masked_byte_spec(bytes, n, ((n * 5) / 8) + 1) == 0u8
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let next = i + 1;
    let start = next * 8;
    let c = bytes[next as int];
    div_mod_decompose_8(bit);
    assert((bit % 8) < 8) by(nonlinear_arith);
    assert(j <= 7) by(nonlinear_arith)
        requires j == bit % 8, (bit % 8) < 8;
    assert(bit == 8 * i + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8;
    assert(n * 5 < start) by(nonlinear_arith)
        requires
            bit == n * 5,
            bit == 8 * i + j,
            next == i + 1,
            start == next * 8,
            j <= 7;
    assert(written_bits_in_byte_spec(n, next) == 0);
    assert(low_mask_spec(written_bits_in_byte_spec(n, next)) == 0u8);
    assert(prefix_masked_byte_spec(bytes, n, next) == (((c as u64) & 0u64) as u8));
    assert((((c as u64) & 0u64) as u8) == 0u8) by(bit_vector);
}

proof fn past_byte_full(bytes: Seq<u8>, n: nat, idx: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        idx < bytes.len(),
        idx < (n * 5) / 8,
    ensures
        prefix_masked_byte_spec(bytes, n, idx) == bytes[idx as int],
        prefix_masked_byte_spec(bytes, n + 1, idx) == bytes[idx as int],
{
    let b = bytes[idx as int];
    let bit = n * 5;
    assert((idx + 1) * 8 <= bit) by(nonlinear_arith)
        requires idx < bit / 8;
    assert(idx * 8 + 8 <= bit) by(nonlinear_arith)
        requires (idx + 1) * 8 <= bit;
    assert(idx * 8 + 8 <= (n + 1) * 5) by(nonlinear_arith)
        requires idx * 8 + 8 <= bit, bit == n * 5;
    assert(written_bits_in_byte_spec(n, idx) == 8);
    assert(written_bits_in_byte_spec(n + 1, idx) == 8);
    assert(low_mask_spec(written_bits_in_byte_spec(n, idx)) == 255u8);
    assert(low_mask_spec(written_bits_in_byte_spec(n + 1, idx)) == 255u8);
    assert(prefix_masked_byte_spec(bytes, n, idx) == (((b as u64) & 255u64) as u8));
    assert(prefix_masked_byte_spec(bytes, n + 1, idx) == (((b as u64) & 255u64) as u8));
    assert((((b as u64) & 255u64) as u8) == b) by(bit_vector);
}

proof fn future_byte_zero(bytes: Seq<u8>, n: nat, idx: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        idx < bytes.len(),
        idx >= ((n * 5) / 8) + 2,
    ensures
        prefix_masked_byte_spec(bytes, n, idx) == 0u8,
        prefix_masked_byte_spec(bytes, n + 1, idx) == 0u8,
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let start = idx * 8;
    let b = bytes[idx as int];
    div_mod_decompose_8(bit);
    assert((bit % 8) < 8) by(nonlinear_arith);
    assert(j <= 7) by(nonlinear_arith)
        requires j == bit % 8, (bit % 8) < 8;
    assert(bit == 8 * i + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8;
    assert(bit < start) by(nonlinear_arith)
        requires
            bit == 8 * i + j,
            idx >= i + 2,
            j <= 7,
            start == idx * 8;
    assert((n + 1) * 5 < start) by(nonlinear_arith)
        requires
            bit == n * 5,
            bit == 8 * i + j,
            idx >= i + 2,
            j <= 7,
            start == idx * 8;
    assert(written_bits_in_byte_spec(n, idx) == 0);
    assert(written_bits_in_byte_spec(n + 1, idx) == 0);
    assert(low_mask_spec(written_bits_in_byte_spec(n, idx)) == 0u8);
    assert(low_mask_spec(written_bits_in_byte_spec(n + 1, idx)) == 0u8);
    assert(prefix_masked_byte_spec(bytes, n, idx) == (((b as u64) & 0u64) as u8));
    assert(prefix_masked_byte_spec(bytes, n + 1, idx) == (((b as u64) & 0u64) as u8));
    assert((((b as u64) & 0u64) as u8) == 0u8) by(bit_vector);
}

proof fn next_byte_zero_when_no_spill(bytes: Seq<u8>, n: nat)
    requires
        bytes.len() > 0,
        n < encoded_length_spec(bytes.len()),
        (n * 5) % 8 <= 3,
        ((n * 5) / 8) + 1 < bytes.len(),
    ensures
        prefix_masked_byte_spec(bytes, n + 1, ((n * 5) / 8) + 1) == 0u8
{
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let next = i + 1;
    let start = next * 8;
    let c = bytes[next as int];
    div_mod_decompose_8(bit);
    assert(bit == 8 * i + j) by(nonlinear_arith)
        requires
            bit == 8 * (bit / 8) + (bit % 8),
            i == bit / 8,
            j == bit % 8;
    assert((n + 1) * 5 <= start) by(nonlinear_arith)
        requires
            bit == n * 5,
            bit == 8 * i + j,
            next == i + 1,
            start == next * 8,
            j <= 3;
    assert(written_bits_in_byte_spec(n + 1, next) == 0);
    assert(low_mask_spec(written_bits_in_byte_spec(n + 1, next)) == 0u8);
    assert(prefix_masked_byte_spec(bytes, n + 1, next) == (((c as u64) & 0u64) as u8));
    assert((((c as u64) & 0u64) as u8) == 0u8) by(bit_vector);
}

proof fn reconstruct_prefix_masked(bytes: Seq<u8>, upto: nat)
    requires
        upto <= encoded_length_spec(bytes.len()),
    ensures
        extend_to_spec(reconstruct_prefix_spec(bytes, upto), bytes.len()) == prefix_masked_bytes_spec(bytes, upto),
    decreases upto
{
    if upto == 0 {
        prefix_masked_zero(bytes);
        assert(reconstruct_prefix_spec(bytes, 0) == Seq::<u8>::empty());
        assert(extend_to_spec(reconstruct_prefix_spec(bytes, 0), bytes.len()) == zero_bytes_spec(bytes.len()));
        assert(zero_bytes_spec(bytes.len()) == prefix_masked_bytes_spec(bytes, 0));
    } else {
        let n = (upto - 1) as nat;
        let bit = n * 5;
        let i = bit / 8;
        let j = bit % 8;
        let d = digit_from_bit_spec(bytes, bit);
        let low = decode_low_spec(bit, d);
        let high = decode_high_spec(bit, d);
        let acc = reconstruct_prefix_spec(bytes, n);
        let acc1 = or_extend_at_spec(acc, i, low);
        let result = reconstruct_prefix_spec(bytes, upto);
        reconstruct_prefix_masked(bytes, n);
        chunk_start_in_bounds(bytes.len(), n);
        reconstruct_prefix_len_upper(bytes, n);
        reconstruct_prefix_len_upper(bytes, upto);
        or_extend_at_within_len(acc, bytes.len(), i, low);
        assert(result == decode_step_spec(acc, n, d));
        assert(result == if high == 0u8 { acc1 } else { or_extend_at_spec(acc1, i + 1, high) });
        assert forall|k: int| 0 <= k < bytes.len() implies #[trigger] extend_to_spec(result, bytes.len())[k] == prefix_masked_bytes_spec(bytes, upto)[k] by {
            let idx = k as nat;
            let prev = extend_to_spec(acc, bytes.len())[k];
            let cur = extend_to_spec(acc1, bytes.len())[k];
            let lhs = extend_to_spec(result, bytes.len())[k];
            let masked_n = prefix_masked_byte_spec(bytes, n, idx);
            let masked_u = prefix_masked_byte_spec(bytes, upto, idx);
            assert(prev == prefix_masked_bytes_spec(bytes, n)[k]);
            assert(prefix_masked_bytes_spec(bytes, n)[k] == masked_n);
            assert(prefix_masked_bytes_spec(bytes, upto)[k] == masked_u);
            if idx < i {
                past_byte_full(bytes, n, idx);
                or_extend_at_full_other(acc, bytes.len(), i, low, idx);
                assert(cur == prev);
                if j <= 3 {
                    digit_from_bit_bounded(bytes, bit);
                    decode_high_zero_when_no_spill(bit, d);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                } else if i + 1 == bytes.len() {
                    last_chunk_spill_zero(bytes, n);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                } else if high == 0u8 {
                    assert(result == acc1);
                    assert(lhs == cur);
                } else {
                    or_extend_at_within_len(acc1, bytes.len(), i + 1, high);
                    or_extend_at_full_other(acc1, bytes.len(), i + 1, high, idx);
                    assert(result == or_extend_at_spec(acc1, i + 1, high));
                    assert(lhs == cur);
                }
                assert(masked_n == masked_u);
                assert(lhs == prefix_masked_bytes_spec(bytes, upto)[k]);
            } else if idx == i {
                or_extend_at_full_index(acc, bytes.len(), i, low);
                if j <= 3 {
                    digit_from_bit_bounded(bytes, bit);
                    decode_high_zero_when_no_spill(bit, d);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                    current_byte_update_no_spill(bytes, n);
                } else if i + 1 == bytes.len() {
                    last_chunk_spill_zero(bytes, n);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                    current_byte_update_last_spill(bytes, n);
                } else {
                    if high == 0u8 {
                        assert(result == acc1);
                        assert(lhs == cur);
                    } else {
                        or_extend_at_within_len(acc1, bytes.len(), i + 1, high);
                        or_extend_at_full_other(acc1, bytes.len(), i + 1, high, i);
                        assert(result == or_extend_at_spec(acc1, i + 1, high));
                        assert(lhs == cur);
                    }
                    current_byte_update_spill(bytes, n);
                }
                assert(lhs == prefix_masked_bytes_spec(bytes, upto)[k]);
            } else if idx == i + 1 {
                assert(i + 1 < bytes.len());
                next_byte_zero_before_step(bytes, n);
                or_extend_at_full_other(acc, bytes.len(), i, low, idx);
                assert(cur == prev);
                assert(masked_n == 0u8);
                if j <= 3 {
                    digit_from_bit_bounded(bytes, bit);
                    decode_high_zero_when_no_spill(bit, d);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                    next_byte_zero_when_no_spill(bytes, n);
                    assert(masked_u == 0u8);
                    assert(lhs == prefix_masked_bytes_spec(bytes, upto)[k]);
                } else {
                    next_byte_update_spill(bytes, n);
                    if high == 0u8 {
                        assert(result == acc1);
                        assert(lhs == cur);
                        assert(masked_u == (((masked_n as u64) | (high as u64)) as u8));
                        assert(masked_u == 0u8) by(bit_vector)
                            requires masked_n == 0u8, high == 0u8, masked_u == (((masked_n as u64) | (high as u64)) as u8);
                    } else {
                        or_extend_at_within_len(acc1, bytes.len(), i + 1, high);
                        or_extend_at_full_index(acc1, bytes.len(), i + 1, high);
                        assert(result == or_extend_at_spec(acc1, i + 1, high));
                        assert(lhs == (((cur as u64) | (high as u64)) as u8));
                    }
                    assert(lhs == prefix_masked_bytes_spec(bytes, upto)[k]);
                }
            } else {
                assert(idx > i + 1) by(nonlinear_arith)
                    requires !(idx < i), idx != i, idx != i + 1;
                future_byte_zero(bytes, n, idx);
                or_extend_at_full_other(acc, bytes.len(), i, low, idx);
                assert(cur == prev);
                assert(masked_n == 0u8);
                if j <= 3 {
                    digit_from_bit_bounded(bytes, bit);
                    decode_high_zero_when_no_spill(bit, d);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                } else if i + 1 == bytes.len() {
                    last_chunk_spill_zero(bytes, n);
                    assert(high == 0u8);
                    assert(result == acc1);
                    assert(lhs == cur);
                } else if high == 0u8 {
                    assert(result == acc1);
                    assert(lhs == cur);
                } else {
                    or_extend_at_within_len(acc1, bytes.len(), i + 1, high);
                    or_extend_at_full_other(acc1, bytes.len(), i + 1, high, idx);
                    assert(result == or_extend_at_spec(acc1, i + 1, high));
                    assert(lhs == cur);
                }
                assert(masked_u == 0u8);
                assert(lhs == prefix_masked_bytes_spec(bytes, upto)[k]);
            }
        }
        extend_to_len(result, bytes.len());
        assert(prefix_masked_bytes_spec(bytes, upto).len() == bytes.len());
        assert(extend_to_spec(result, bytes.len()).len() == bytes.len());
        assert(extend_to_spec(result, bytes.len()) == prefix_masked_bytes_spec(bytes, upto));
    }
}


// r[verify nix-base32.bijectivity]
/// Canonical round-trip: decode(encode(bytes)) returns the original bytes.
pub proof fn encode_decode_roundtrip(bytes: Seq<u8>)
    ensures decode_spec(encode_spec(bytes)) == Some(bytes)
{
    let out_len = encoded_length_spec(bytes.len());
    let rec = reconstruct_prefix_spec(bytes, out_len);
    encode_output_length(bytes);
    decode_prefix_of_encode(bytes, out_len);
    reconstruct_prefix_masked(bytes, out_len);
    reconstruct_full_len(bytes);
    prefix_masked_full(bytes);
    assert(decode_spec(encode_spec(bytes)) == decode_prefix_spec(encode_spec(bytes), out_len));
    assert(extend_to_spec(rec, bytes.len()) == prefix_masked_bytes_spec(bytes, out_len));
    assert(prefix_masked_bytes_spec(bytes, out_len) == bytes);
    assert(rec.len() == bytes.len());
    extend_to_same_len(rec);
    assert(extend_to_spec(rec, bytes.len()) == extend_to_spec(rec, rec.len()));
    assert(extend_to_spec(rec, bytes.len()) == rec);
    assert(rec == bytes);
}

// r[verify nix-base32.bijectivity]
/// Encode is injective by the decode-after-encode round-trip.
pub proof fn encode_injective(a: Seq<u8>, b: Seq<u8>)
    requires encode_spec(a) == encode_spec(b)
    ensures a == b
{
    encode_decode_roundtrip(a);
    encode_decode_roundtrip(b);
}

proof fn encode_chunk_window_fits(len: nat, n: nat)
    requires
        len <= ((usize::MAX as int) - 4) / 8,
        n < encoded_length_spec(len),
    ensures
        n * 5 + 4 <= usize::MAX as nat,
{
    encoded_length_ceil(len);
    assert(len > 0);
    chunk_start_in_bounds(len, n);
    assert(len * 8 + 4 <= usize::MAX as nat) by(nonlinear_arith)
        requires len <= ((usize::MAX as int) - 4) / 8;
    assert(n * 5 + 4 < len * 8 + 4) by(nonlinear_arith)
        requires n * 5 < len * 8;
}

proof fn mul5_usize_nat(n: usize)
    requires (n as nat) * 5 + 4 <= usize::MAX as nat
    ensures
        ((n * 5) as nat) == (n as nat) * 5,
        ((n * 5 + 1) as nat) == (n as nat) * 5 + 1,
        ((n * 5 + 2) as nat) == (n as nat) * 5 + 2,
        ((n * 5 + 3) as nat) == (n as nat) * 5 + 3,
        ((n * 5 + 4) as nat) == (n as nat) * 5 + 4,
{}

proof fn encoded_length_exec_eq(len: usize)
    requires len as nat <= ((usize::MAX as int) - 4) / 8
    ensures ((len * 8 + 4) / 5) as nat == encoded_length_spec(len as nat)
{}

fn bit_at_padded_exec(bytes: &Vec<u8>, pos: usize) -> (result: u8)
    requires bytes@.len() <= ((usize::MAX as int) - 4) / 8
    ensures
        result == bit_at_padded_spec(bytes@, pos as nat),
        result <= 1u8,
{
    let shift: u64 = (pos % 8) as u64;
    let result = if pos < bytes.len() * 8 {
        (((bytes[pos / 8] as u64) >> shift) & 1u64) as u8
    } else {
        0u8
    };
    proof {
        if pos < bytes.len() * 8 {
            assert((pos as nat) < ((bytes.len() as nat) * 8));
        } else {
            assert((pos as nat) >= ((bytes.len() as nat) * 8));
        }
        bit_at_padded_bounded(bytes@, pos as nat);
    }
    result
}

#[allow(clippy::cast_possible_truncation)]
fn encode_digit(bytes: &Vec<u8>, n: usize) -> (digit: u8)
    requires
        bytes@.len() <= ((usize::MAX as int) - 4) / 8,
        n < encoded_length_spec(bytes@.len()),
    ensures
        digit == digit_from_bit_spec(bytes@, (n as nat) * 5),
        digit < 32u8,
{
    proof {
        encode_chunk_window_fits(bytes@.len(), n as nat);
    }
    let bit = n * 5;
    let b0 = bit_at_padded_exec(bytes, bit);
    let b1 = bit_at_padded_exec(bytes, bit + 1);
    let b2 = bit_at_padded_exec(bytes, bit + 2);
    let b3 = bit_at_padded_exec(bytes, bit + 3);
    let b4 = bit_at_padded_exec(bytes, bit + 4);
    let digit = ((b0 as u64)
        | ((b1 as u64) << 1u64)
        | ((b2 as u64) << 2u64)
        | ((b3 as u64) << 3u64)
        | ((b4 as u64) << 4u64)) as u8;
    proof {
        let start = (n as nat) * 5;
        mul5_usize_nat(n);
        assert(digit == digit_from_bit_spec(bytes@, start));
        digit_from_bit_bounded(bytes@, start);
    }
    digit
}

fn alphabet_char(digit: u8) -> (result: u8)
    requires digit < 32u8
    ensures result == alphabet_spec()[digit as int]
{
    if digit == 0u8 { 48u8 }
    else if digit == 1u8 { 49u8 }
    else if digit == 2u8 { 50u8 }
    else if digit == 3u8 { 51u8 }
    else if digit == 4u8 { 52u8 }
    else if digit == 5u8 { 53u8 }
    else if digit == 6u8 { 54u8 }
    else if digit == 7u8 { 55u8 }
    else if digit == 8u8 { 56u8 }
    else if digit == 9u8 { 57u8 }
    else if digit == 10u8 { 97u8 }
    else if digit == 11u8 { 98u8 }
    else if digit == 12u8 { 99u8 }
    else if digit == 13u8 { 100u8 }
    else if digit == 14u8 { 102u8 }
    else if digit == 15u8 { 103u8 }
    else if digit == 16u8 { 104u8 }
    else if digit == 17u8 { 105u8 }
    else if digit == 18u8 { 106u8 }
    else if digit == 19u8 { 107u8 }
    else if digit == 20u8 { 108u8 }
    else if digit == 21u8 { 109u8 }
    else if digit == 22u8 { 110u8 }
    else if digit == 23u8 { 112u8 }
    else if digit == 24u8 { 113u8 }
    else if digit == 25u8 { 114u8 }
    else if digit == 26u8 { 115u8 }
    else if digit == 27u8 { 118u8 }
    else if digit == 28u8 { 119u8 }
    else if digit == 29u8 { 120u8 }
    else if digit == 30u8 { 121u8 }
    else { 122u8 }
}

// r[impl nix-base32.encode]
/// Encode bytes using Nix Base32.
#[allow(clippy::manual_div_ceil)]
pub fn encode(bytes: &Vec<u8>) -> (result: Vec<u8>)
    requires bytes@.len() <= ((usize::MAX as int) - 4) / 8
    ensures result@ == encode_spec(bytes@)
{
    if bytes.is_empty() {
        proof {
            assert(encode_spec(bytes@) == Seq::<u8>::empty());
        }
        return Vec::new();
    }

    let out_len = (bytes.len() * 8 + 4) / 5;
    proof {
        encoded_length_exec_eq(bytes.len());
        assert((bytes.len() as nat) == bytes@.len());
        assert(out_len as nat == encoded_length_spec(bytes@.len()));
    }
    let mut result = Vec::with_capacity(out_len);
    let mut idx: usize = 0;
    while idx < out_len
        invariant
            bytes@.len() <= ((usize::MAX as int) - 4) / 8,
            bytes.len() > 0,
            out_len as nat == encoded_length_spec(bytes@.len()),
            0 <= idx <= out_len,
            result@.len() == idx as int,
            forall|j: int| 0 <= j < idx as int ==> result@[j] == encode_char_spec(bytes@, out_len as nat, j as nat),
        decreases out_len - idx
    {
        let n = out_len - 1 - idx;
        let digit = encode_digit(bytes, n);
        let c = alphabet_char(digit);
        proof {
            assert((out_len - 1 - idx) as nat == out_len as nat - 1 - idx as nat) by(nonlinear_arith)
                requires idx < out_len;
            assert(c == encode_char_spec(bytes@, out_len as nat, idx as nat));
        }
        result.push(c);
        idx = idx + 1;
        proof {
            assert forall|j: int| 0 <= j < idx as int implies result@[j] == encode_char_spec(bytes@, out_len as nat, j as nat) by {
                if j < idx as int - 1 {
                } else {
                    assert(j == idx as int - 1) by(nonlinear_arith)
                        requires 0 <= j < idx as int, !(j < idx as int - 1);
                }
            }
        }
    }
    proof {
        assert forall|j: int| 0 <= j < out_len as int implies result@[j] == encode_char_spec(bytes@, out_len as nat, j as nat) by {
        }
        assert(result@ == encode_prefix_spec(bytes@, out_len as nat, out_len as nat));
        assert(result@ == encode_spec(bytes@));
    }
    result
}

proof fn decode_chunk_window_fits(len: nat, n: nat)
    requires
        len <= ((usize::MAX as int) - 4) / 5 + 1,
        n < len,
    ensures
        n * 5 + 4 <= usize::MAX as nat,
{
    assert(len > 0);
    assert(n <= len - 1) by(nonlinear_arith)
        requires n < len, len > 0;
    assert(n * 5 + 4 <= (len - 1) * 5 + 4) by(nonlinear_arith)
        requires n <= len - 1;
    assert((len - 1) * 5 + 4 <= usize::MAX as nat) by(nonlinear_arith)
        requires len <= ((usize::MAX as int) - 4) / 5 + 1;
}

fn extend_to_exec(acc: &Vec<u8>, len: usize) -> (result: Vec<u8>)
    requires acc@.len() <= len as nat
    ensures result@ == extend_to_spec(acc@, len as nat)
{
    let mut result = Vec::with_capacity(len);
    let mut i: usize = 0;
    while i < acc.len()
        invariant
            acc@.len() <= len as nat,
            0 <= i <= acc.len(),
            result@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> result@[j] == acc@[j],
        decreases acc.len() - i
    {
        result.push(acc[i]);
        i = i + 1;
    }
    let mut j: usize = acc.len();
    while j < len
        invariant
            acc@.len() <= len as nat,
            acc.len() <= j <= len,
            result@.len() == j as int,
            forall|k: int| 0 <= k < acc.len() as int ==> result@[k] == acc@[k],
            forall|k: int| acc.len() as int <= k < j as int ==> result@[k] == 0u8,
        decreases len - j
    {
        result.push(0u8);
        j = j + 1;
    }
    proof {
        extend_to_len(acc@, len as nat);
        assert forall|k: int| 0 <= k < len as int implies result@[k] == extend_to_spec(acc@, len as nat)[k] by {
            let kk = k as nat;
            extend_to_index(acc@, len as nat, kk);
            if k < acc.len() as int {
            } else {
                assert(result@[k] == 0u8);
            }
        }
        assert(result@.len() == extend_to_spec(acc@, len as nat).len());
        assert(result@ == extend_to_spec(acc@, len as nat));
    }
    result
}

#[allow(clippy::cast_possible_truncation)]
fn or_extend_at_exec(acc: &Vec<u8>, idx: usize, val: u8) -> (result: Vec<u8>)
    requires idx < usize::MAX
    ensures result@ == or_extend_at_spec(acc@, idx as nat, val)
{
    let len = if acc.len() <= idx { idx + 1 } else { acc.len() };
    let mut result = extend_to_exec(acc, len);
    let cur = result[idx];
    proof {
        assert(cur == extend_to_spec(acc@, len as nat)[idx as int]);
    }
    let new = cur | val;
    result.set(idx, new);
    proof {
        if acc@.len() <= idx as nat {
            assert(len as nat == idx as nat + 1);
            assert(result@ == extend_to_spec(acc@, idx as nat + 1).update(idx as int, new));
        } else {
            assert(len as nat == acc@.len());
            extend_to_same_len(acc@);
            assert(extend_to_spec(acc@, len as nat) == acc@);
            assert(result@ == acc@.update(idx as int, new));
        }
        assert(result@ == or_extend_at_spec(acc@, idx as nat, val));
    }
    result
}

fn decode_char_exec(c: u8) -> (result: Option<u8>)
    ensures match result {
        Some(d) => decode_char_spec(c) == Some(d),
        None => decode_char_spec(c) == None::<u8>,
    }
{
    if c == 48u8 { Some(0u8) }
    else if c == 49u8 { Some(1u8) }
    else if c == 50u8 { Some(2u8) }
    else if c == 51u8 { Some(3u8) }
    else if c == 52u8 { Some(4u8) }
    else if c == 53u8 { Some(5u8) }
    else if c == 54u8 { Some(6u8) }
    else if c == 55u8 { Some(7u8) }
    else if c == 56u8 { Some(8u8) }
    else if c == 57u8 { Some(9u8) }
    else if c == 97u8 { Some(10u8) }
    else if c == 98u8 { Some(11u8) }
    else if c == 99u8 { Some(12u8) }
    else if c == 100u8 { Some(13u8) }
    else if c == 102u8 { Some(14u8) }
    else if c == 103u8 { Some(15u8) }
    else if c == 104u8 { Some(16u8) }
    else if c == 105u8 { Some(17u8) }
    else if c == 106u8 { Some(18u8) }
    else if c == 107u8 { Some(19u8) }
    else if c == 108u8 { Some(20u8) }
    else if c == 109u8 { Some(21u8) }
    else if c == 110u8 { Some(22u8) }
    else if c == 112u8 { Some(23u8) }
    else if c == 113u8 { Some(24u8) }
    else if c == 114u8 { Some(25u8) }
    else if c == 115u8 { Some(26u8) }
    else if c == 118u8 { Some(27u8) }
    else if c == 119u8 { Some(28u8) }
    else if c == 120u8 { Some(29u8) }
    else if c == 121u8 { Some(30u8) }
    else if c == 122u8 { Some(31u8) }
    else { None }
}

fn decode_step_exec(acc: &Vec<u8>, n: usize, digit: u8) -> (result: Vec<u8>)
    requires
        (n as nat) * 5 + 4 <= usize::MAX as nat,
        digit < 32u8,
    ensures result@ == decode_step_spec(acc@, n as nat, digit)
{
    proof {
        mul5_usize_nat(n);
    }
    let bit = n * 5;
    let i = bit / 8;
    let j = bit % 8;
    let low = (((digit as u64) << (j as u64)) & 0xffu64) as u8;
    proof {
        assert(i < usize::MAX) by(nonlinear_arith)
            requires
                (n as nat) * 5 + 4 <= usize::MAX as nat,
                i == bit / 8,
                (bit as nat) == (n as nat) * 5;
    }
    let acc1 = or_extend_at_exec(acc, i, low);
    let high = if j == 0 { 0u8 } else { digit >> (8usize - j) };
    if high == 0u8 {
        proof {
            assert((bit as nat) == (n as nat) * 5);
            assert(i as nat == ((n as nat) * 5) / 8) by(nonlinear_arith)
                requires i == bit / 8, (bit as nat) == (n as nat) * 5;
            assert(low == decode_low_spec((n as nat) * 5, digit));
            assert(high == decode_high_spec((n as nat) * 5, digit));
            assert(acc1@ == or_extend_at_spec(acc@, i as nat, low));
            assert(acc1@ == decode_step_spec(acc@, n as nat, digit));
        }
        acc1
    } else {
        proof {
            assert(i + 1 < usize::MAX) by(nonlinear_arith)
                requires
                    (n as nat) * 5 + 4 <= usize::MAX as nat,
                    i == bit / 8,
                    (bit as nat) == (n as nat) * 5;
        }
        let acc2 = or_extend_at_exec(&acc1, i + 1, high);
        proof {
            assert((bit as nat) == (n as nat) * 5);
            assert(i as nat == ((n as nat) * 5) / 8) by(nonlinear_arith)
                requires i == bit / 8, (bit as nat) == (n as nat) * 5;
            assert(low == decode_low_spec((n as nat) * 5, digit));
            assert(high == decode_high_spec((n as nat) * 5, digit));
            assert(acc1@ == or_extend_at_spec(acc@, i as nat, low));
            assert(acc2@ == or_extend_at_spec(acc1@, i as nat + 1, high));
            assert(acc2@ == decode_step_spec(acc@, n as nat, digit));
        }
        acc2
    }
}

fn decode_prefix_exec(chars: &Vec<u8>, upto: usize) -> (result: Option<Vec<u8>>)
    requires
        chars@.len() <= ((usize::MAX as int) - 4) / 5 + 1,
        upto <= chars.len(),
    ensures match result {
        Some(v) => decode_prefix_spec(chars@, upto as nat) == Some(v@),
        None => decode_prefix_spec(chars@, upto as nat) == None::<Seq<u8>>,
    }
    decreases upto
{
    if upto == 0 {
        let out = Vec::new();
        proof {
            assert(out@ == Seq::<u8>::empty());
            assert(decode_prefix_spec(chars@, 0) == Some(out@));
        }
        Some(out)
    } else {
        let prev = decode_prefix_exec(chars, upto - 1);
        match prev {
            None => {
                proof {
                    assert(decode_prefix_spec(chars@, (upto - 1) as nat) == None::<Seq<u8>>);
                    assert(decode_prefix_spec(chars@, upto as nat) == None::<Seq<u8>>);
                }
                None
            }
            Some(acc) => {
                let c = chars[chars.len() - upto];
                let digit = decode_char_exec(c);
                match digit {
                    None => {
                        proof {
                            assert(c == chars@[(chars.len() - upto) as int]);
                            assert(decode_prefix_spec(chars@, (upto - 1) as nat) == Some(acc@));
                            assert(decode_char_spec(c) == None::<u8>);
                            assert(decode_prefix_spec(chars@, upto as nat) == None::<Seq<u8>>);
                        }
                        None
                    }
                    Some(d) => {
                        proof {
                            assert(((upto - 1) as nat) < (chars.len() as nat));
                            assert((chars.len() as nat) == chars@.len());
                            decode_chunk_window_fits(chars.len() as nat, (upto - 1) as nat);
                        }
                        let next = decode_step_exec(&acc, upto - 1, d);
                        proof {
                            assert(c == chars@[(chars.len() - upto) as int]);
                            assert(decode_prefix_spec(chars@, (upto - 1) as nat) == Some(acc@));
                            assert(decode_char_spec(c) == Some(d));
                            assert(next@ == decode_step_spec(acc@, (upto - 1) as nat, d));
                            assert(decode_prefix_spec(chars@, upto as nat) == Some(next@));
                        }
                        Some(next)
                    }
                }
            }
        }
    }
}

// r[impl nix-base32.decode]
/// Decode a Nix Base32 string.
pub fn decode(chars: &Vec<u8>) -> (result: Option<Vec<u8>>)
    requires chars@.len() <= ((usize::MAX as int) - 4) / 5 + 1
    ensures match result {
        Some(v) => decode_spec(chars@) == Some(v@),
        None => decode_spec(chars@) == None::<Seq<u8>>,
    }
{
    let result = decode_prefix_exec(chars, chars.len());
    proof {
        assert(chars.len() as nat == chars@.len());
        match &result {
            Some(v) => assert(decode_spec(chars@) == Some(v@)),
            None => assert(decode_spec(chars@) == None::<Seq<u8>>),
        }
    }
    result
}

} // verus!

#[cfg(test)]
mod tests {
    use std::process::Command;

    use proptest::prelude::*;

    use super::*;

    fn v(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    fn hex_of(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            out.push(HEX[(b >> 4u8) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    }

    fn nix32_sha256(hex: &str) -> Option<String> {
        let output = Command::new("nix").args(["hash", "to-base32", "--type", "sha256", hex]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        Some(stdout.trim().to_owned())
    }

    // r[verify nix-base32.encode]
    #[test]
    fn test_encode_known_small_values() {
        assert_eq!(encode(&vec![]), v(""));
        assert_eq!(encode(&vec![0x00]), v("00"));
        assert_eq!(encode(&vec![0x1f]), v("0z"));
        assert_eq!(encode(&v("abc")), v("66qk1"));
    }

    // r[verify nix-base32.decode]
    #[test]
    fn test_decode_known_small_values() {
        assert_eq!(decode(&v("")), Some(vec![]));
        assert_eq!(decode(&v("00")), Some(vec![0x00]));
        assert_eq!(decode(&v("0z")), Some(vec![0x1f]));
        assert_eq!(decode(&v("0")), Some(vec![0x00]));
    }

    // r[verify nix-base32.bijectivity]
    #[test]
    fn test_encode_decode_roundtrip_examples() {
        for bytes in [vec![], vec![0], vec![0x1f], v("abc"), (0u8..32).collect()] {
            assert_eq!(decode(&encode(&bytes)), Some(bytes));
        }
    }

    // r[verify nix-base32.output-length]
    #[allow(clippy::manual_div_ceil)]
    #[test]
    fn test_encode_output_length() {
        for len in 0usize..20 {
            let bytes = vec![0u8; len];
            assert_eq!(encode(&bytes).len(), (len * 8 + 4) / 5);
        }
    }

    // r[verify nix-base32.encode]
    #[test]
    fn test_nix_cli_oracle_known_sha256_zero() {
        let bytes = vec![0u8; 32];
        let ours = String::from_utf8(encode(&bytes)).unwrap();
        if let Some(theirs) = nix32_sha256(&hex_of(&bytes)) {
            assert_eq!(theirs, ours);
        }
    }

    proptest! {
        // r[verify nix-base32.bijectivity]
        #[test]
        fn prop_encode_decode_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            prop_assert_eq!(decode(&encode(&bytes)), Some(bytes));
        }

        // r[verify nix-base32.output-length]
        #[test]
        #[allow(clippy::manual_div_ceil)]
        fn prop_output_length(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            prop_assert_eq!(encode(&bytes).len(), (bytes.len() * 8 + 4) / 5);
        }

        // r[verify nix-base32.bijectivity]
        #[test]
        fn prop_encode_injective(
            a in proptest::collection::vec(any::<u8>(), 0..32),
            b in proptest::collection::vec(any::<u8>(), 0..32),
        ) {
            let ea = encode(&a);
            let eb = encode(&b);
            if ea == eb {
                prop_assert_eq!(a, b);
            }
        }

        // r[verify nix-base32.encode]
        #[test]
        fn prop_nix_cli_oracle_sha256(bytes in proptest::collection::vec(any::<u8>(), 32..33)) {
            let ours = String::from_utf8(encode(&bytes)).unwrap();
            if let Some(theirs) = nix32_sha256(&hex_of(&bytes)) {
                prop_assert_eq!(ours, theirs);
            }
        }
    }
}
