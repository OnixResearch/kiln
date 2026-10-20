//! Final UCAN `1.0.0` wire-profile shell.
//!
//! This std module owns the canonical DAG-CBOR subset, the exact IPv1
//! protocol CID (SHA-256), the Varsig-subset envelope, the first admitted
//! Ed25519 adapter, and caller-owned signer and DID-key resolver ports. It
//! performs no I/O, clock reads, nonce generation, key custody, or ambient
//! backend access. The no-std profile core in `ucan-core` owns the domain
//! types and field admission; this module owns bytes and cryptography.
//!
//! The envelope is a canonical CBOR array of two elements: the 64-byte
//! Ed25519 signature and the canonical DAG-CBOR payload map. The signature
//! covers the canonical payload-map bytes exactly. `alg` is fixed to
//! `Ed25519` and `enc` to `DAG-CBOR`; the payload map carries the nominal
//! `tag` and `version` so kind substitution fails closed.

#![allow(
    clippy::module_name_repetitions,
    // The CBOR and multiformat framing code below performs deliberate
    // narrowing casts, unsigned magnitude handling, and decimal length
    // literals; the surrounding canonical checks verify every value.
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::checked_conversions,
    clippy::decimal_literal_representation,
    clippy::needless_return,
    clippy::type_complexity,
    // Module documentation intentionally names protocol terms without code
    // ticks and keeps reference helpers below the code that uses them.
    clippy::doc_markdown,
    clippy::items_after_statements
)]

pub use core::FieldAdmission;
pub use core::IpldValue;
pub use core::SpecKind;
pub use core::WireError;

use base64::Engine as _;
use sha2::Digest as _;
use sha2::Sha256;
use ucan_core::profile::v1 as core;

const MULTICODEC_DAG_CBOR: u8 = 0x71;
const MULTIHASH_SHA2_256: u8 = 0x12;
const SHA256_DIGEST_LEN: usize = 32;
const CIDV1_VERSION_BYTE: u8 = 0x01;
const ED25519_SIGNATURE_LEN: usize = 64;
const ED25519_PUBLIC_KEY_LEN: usize = 32;
/// Bound on a decoded top-level envelope or payload in bytes.
pub const MAX_WIRE_INPUT_BYTES: usize = 1 << 20;
/// Bound on the number of elements in any decoded collection.
const MAX_WIRE_ITEMS: usize = 1_024;
/// Bound on parsed proof references per payload.
pub const MAX_PROOFS_IN_PAYLOAD: usize = core::MAX_PROOF_REFERENCES;
/// Base58 Bitcoin alphabet (multibase `z` prefix).
const BASE58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE58_MULTIBASE_PREFIX: u8 = b'z';

/// A canonical-CBOR subset value used by the wire-profile payload handling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Cbor {
    UInt(u64),
    NegInt(i64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Self>),
    Map(Vec<(String, Self)>),
    Null,
    Bool(bool),
}

impl Cbor {
    #[must_use]
    pub const fn cbor_len(&self) -> usize {
        match self {
            Self::Bytes(bytes) => bytes.len(),
            Self::Text(text) => text.len(),
            Self::Array(items) => items.len(),
            Self::Map(items) => items.len(),
            _ => 1,
        }
    }
}

/// Decode a canonical-CBOR subset value from `input`, consuming all bytes.
///
/// # Errors
///
/// Returns [`WireError`] for indefinite forms, non-minimal integers,
/// duplicate or unsorted map keys, unknown tags, depth or item overruns,
/// and trailing bytes.
pub fn decode_payload_bytes_to_values(input: &[u8]) -> Result<Vec<(String, Cbor)>, WireError> {
    let mut decoder = StrictDecoder {
        input,
        offset: 0,
        depth: 0,
    };
    let value = decoder.read_value()?;
    if decoder.offset != input.len() {
        return Err(WireError::UnsupportedTag {
            tag: "trailing-bytes".to_owned(),
        });
    }
    match value {
        Cbor::Map(items) => Ok(items),
        _ => Err(WireError::UnsupportedTag {
            tag: "payload-top-level-map".to_owned(),
        }),
    }
}

/// Encode a payload field list as canonical DAG-CBOR with sorted keys.
#[must_use]
pub fn encode_payload_fields(fields: Vec<(String, Cbor)>) -> Vec<u8> {
    let mut items = fields;
    items.sort_by(|left, right| canonical_key_order(&left.0, &right.0));
    encode_value(&Cbor::Map(items))
}

fn canonical_key_order(left: &str, right: &str) -> ::core::cmp::Ordering {
    let left_bytes = encode_text(left);
    let right_bytes = encode_text(right);
    left_bytes.cmp(&right_bytes)
}

struct StrictDecoder<'a> {
    input: &'a [u8],
    offset: usize,
    depth: usize,
}

impl StrictDecoder<'_> {
    fn read_byte(&mut self) -> Result<u8, WireError> {
        let byte = *self.input.get(self.offset).ok_or_else(|| WireError::UnsupportedTag {
            tag: "truncated-input".to_owned(),
        })?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_exact(&mut self, len: usize) -> Result<&[u8], WireError> {
        let end = self.offset.checked_add(len).ok_or_else(|| WireError::UnsupportedTag {
            tag: "length-overflow".to_owned(),
        })?;
        if end > self.input.len() {
            return Err(WireError::UnsupportedTag {
                tag: "truncated-input".to_owned(),
            });
        }
        let slice = &self.input[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn read_initially(&mut self, additional: u8) -> Result<u64, WireError> {
        if additional < 24 {
            return Ok(u64::from(additional));
        }
        match additional {
            24 => {
                let value = u64::from(self.read_byte()?);
                if value < 24 {
                    return Err(WireError::UnsupportedTag {
                        tag: "non-minimal-uint".to_owned(),
                    });
                }
                Ok(value)
            }
            25 => {
                let bytes = self.read_exact(2)?;
                let value = u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                if value < 256 {
                    return Err(WireError::UnsupportedTag {
                        tag: "non-minimal-uint".to_owned(),
                    });
                }
                Ok(value)
            }
            26 => {
                let bytes = self.read_exact(4)?;
                let value = u64::from(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                if value < 65_536 {
                    return Err(WireError::UnsupportedTag {
                        tag: "non-minimal-uint".to_owned(),
                    });
                }
                Ok(value)
            }
            27 => {
                let bytes = self.read_exact(8)?;
                let value = u64::from_be_bytes(bytes.try_into().expect("eight bytes"));
                if value < 4_294_967_296 {
                    return Err(WireError::UnsupportedTag {
                        tag: "non-minimal-uint".to_owned(),
                    });
                }
                Ok(value)
            }
            31 => {
                return Err(WireError::UnsupportedTag {
                    tag: "indefinite-form".to_owned(),
                });
            }
            _ => {
                return Err(WireError::UnsupportedTag {
                    tag: "unexpected-additional".to_owned(),
                });
            }
        }
    }

    #[allow(clippy::too_many_lines)] // canonical decoder branches are intentionally exhaustive and linear
    fn read_value(&mut self) -> Result<Cbor, WireError> {
        if self.depth > core::MAX_IPLD_VALUE_DEPTH {
            return Err(WireError::ValueTooDeep { depth: self.depth });
        }
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let additional = initial & 0x1F;
        match major {
            0 => Ok(Cbor::UInt(self.read_initially(additional)?)),
            1 => {
                let value = self.read_initially(additional)?;
                Ok(Cbor::NegInt(
                    -1_i64
                        - i64::try_from(value).map_err(|_| WireError::UnsupportedTag {
                            tag: "negint-overflow".to_owned(),
                        })?,
                ))
            }
            2 => {
                let len = usize::try_from(self.read_initially(additional)?).map_err(|_| WireError::UnsupportedTag {
                    tag: "length-overflow".to_owned(),
                })?;
                Ok(Cbor::Bytes(self.read_exact(len)?.to_vec()))
            }
            3 => {
                let len = usize::try_from(self.read_initially(additional)?).map_err(|_| WireError::UnsupportedTag {
                    tag: "length-overflow".to_owned(),
                })?;
                let bytes = self.read_exact(len)?;
                let text = ::core::str::from_utf8(bytes).map_err(|_| WireError::UnsupportedTag {
                    tag: "invalid-utf8".to_owned(),
                })?;
                Ok(Cbor::Text(text.to_owned()))
            }
            4 => {
                let len = usize::try_from(self.read_initially(additional)?).map_err(|_| WireError::UnsupportedTag {
                    tag: "length-overflow".to_owned(),
                })?;
                if len > MAX_WIRE_ITEMS {
                    return Err(WireError::ValueTooLarge { items: len });
                }
                let mut items = Vec::with_capacity(len);
                for _ in 0..len {
                    self.depth += 1;
                    let item = self.read_value()?;
                    self.depth -= 1;
                    items.push(item);
                }
                Ok(Cbor::Array(items))
            }
            5 => {
                let len = usize::try_from(self.read_initially(additional)?).map_err(|_| WireError::UnsupportedTag {
                    tag: "length-overflow".to_owned(),
                })?;
                if len > MAX_WIRE_ITEMS {
                    return Err(WireError::ValueTooLarge { items: len });
                }
                let mut items = Vec::with_capacity(len);
                let mut previous_key: Option<Vec<u8>> = None;
                for _ in 0..len {
                    self.depth += 1;
                    let key = self.read_value()?;
                    self.depth -= 1;
                    let Cbor::Text(key_text) = key else {
                        return Err(WireError::UnsupportedTag {
                            tag: "non-text-map-key".to_owned(),
                        });
                    };
                    let key_bytes = encode_text(&key_text);
                    if let Some(previous) = &previous_key {
                        if previous == &key_bytes {
                            return Err(WireError::UnsupportedTag {
                                tag: "duplicate-map-key".to_owned(),
                            });
                        }
                        if previous.cmp(&key_bytes) == ::core::cmp::Ordering::Greater {
                            return Err(WireError::UnsupportedTag {
                                tag: "unsorted-map-key".to_owned(),
                            });
                        }
                    }
                    previous_key = Some(key_bytes);
                    self.depth += 1;
                    let value = self.read_value()?;
                    self.depth -= 1;
                    items.push((key_text, value));
                }
                Ok(Cbor::Map(items))
            }
            6 => Err(WireError::UnsupportedTag {
                tag: "unsupported-tag-value".to_owned(),
            }),
            7 => match additional {
                20 => Ok(Cbor::Bool(false)),
                21 => Ok(Cbor::Bool(true)),
                22 => Ok(Cbor::Null),
                27 => Err(WireError::UnsupportedTag {
                    tag: "float64-unsupported".to_owned(),
                }),
                _ => Err(WireError::UnsupportedTag {
                    tag: "unsupported-simple".to_owned(),
                }),
            },
            _ => Err(WireError::UnsupportedTag {
                tag: "invalid-major-type".to_owned(),
            }),
        }
    }
}

fn encode_value(value: &Cbor) -> Vec<u8> {
    match value {
        Cbor::UInt(value) => encode_int(0, *value),
        Cbor::NegInt(value) => encode_int(1, (-1_i64 - value) as u64),
        Cbor::Bytes(bytes) => {
            let mut output = encode_head(2, bytes.len());
            output.extend_from_slice(bytes);
            output
        }
        Cbor::Text(text) => {
            let mut output = encode_head(3, text.len());
            output.extend_from_slice(text.as_bytes());
            output
        }
        Cbor::Array(items) => {
            let mut output = encode_head(4, items.len());
            for item in items {
                output.extend(encode_value(item));
            }
            output
        }
        Cbor::Map(items) => {
            let mut sorted = items.clone();
            sorted.sort_by(|left, right| canonical_key_order(&left.0, &right.0));
            let mut output = encode_head(5, sorted.len());
            for (key, item) in &sorted {
                output.extend(encode_text(key));
                output.extend(encode_value(item));
            }
            output
        }
        Cbor::Null => vec![0xF6],
        Cbor::Bool(false) => vec![0xF4],
        Cbor::Bool(true) => vec![0xF5],
    }
}

fn encode_head(major: u8, len: usize) -> Vec<u8> {
    let mut output = Vec::new();
    if len < 24 {
        output.push((major << 5) | (len as u8));
    } else if len <= u8::MAX as usize {
        output.push((major << 5) | 0x18);
        output.push(len as u8);
    } else if len <= u16::MAX as usize {
        output.push((major << 5) | 0x19);
        output.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= u32::MAX as usize {
        output.push((major << 5) | 0x1A);
        output.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        output.push((major << 5) | 0x1B);
        output.extend_from_slice(&(len as u64).to_be_bytes());
    }
    output
}

fn encode_int(major: u8, magnitude: u64) -> Vec<u8> {
    let mut output = Vec::new();
    if magnitude < 24 {
        output.push((major << 5) | (magnitude as u8));
    } else if magnitude <= u64::from(u8::MAX) {
        output.push((major << 5) | 0x18);
        output.push(magnitude as u8);
    } else if magnitude <= u64::from(u16::MAX) {
        output.push((major << 5) | 0x19);
        output.extend_from_slice(&(magnitude as u16).to_be_bytes());
    } else if magnitude <= u64::from(u32::MAX) {
        output.push((major << 5) | 0x1A);
        output.extend_from_slice(&(magnitude as u32).to_be_bytes());
    } else {
        output.push((major << 5) | 0x1B);
        output.extend_from_slice(&magnitude.to_be_bytes());
    }
    output
}

fn encode_text(text: &str) -> Vec<u8> {
    let mut output = encode_head(3, text.len());
    output.extend_from_slice(text.as_bytes());
    output
}

/// Compute the protocol SHA-256 digest of `payload` bytes.
#[must_use]
pub fn sha256(payload: &[u8]) -> [u8; SHA256_DIGEST_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hasher.finalize().into()
}

/// Build the exact final-profile CIDv1 bytes for a DAG-CBOR payload.
#[must_use]
pub fn cid_v1_of_payload(payload: &[u8]) -> Vec<u8> {
    let mut cid = vec![
        CIDV1_VERSION_BYTE,
        MULTICODEC_DAG_CBOR,
        MULTIHASH_SHA2_256,
        SHA256_DIGEST_LEN as u8,
    ];
    cid.extend_from_slice(&sha256(payload));
    cid
}

/// Encode arbitrary bytes as multibase base58btc.
#[must_use]
pub fn base58btc_encode(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    let leading_zeros = bytes.iter().take_while(|&&byte| byte == 0).count();
    let mut digits = vec![0_u8];
    for &byte in bytes {
        let mut carry = u32::from(byte);
        for digit in &mut digits {
            let value = u32::from(*digit) * 256 + carry;
            *digit = (value % 58) as u8;
            carry = value / 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    digits.reverse();
    let mut output = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        output.push('1');
    }
    for digit in digits {
        output.push(char::from(BASE58_ALPHABET[usize::from(digit)]));
    }
    output
}

/// Decode multibase base58btc bytes (with or without the leading `z`).
///
/// # Errors
///
/// Returns [`WireError`] on an invalid alphabet character or on leading
/// `1` padding that disagrees with the decoded leading zero bytes.
pub fn base58btc_decode(input: &str) -> Result<Vec<u8>, WireError> {
    let value = input.strip_prefix(BASE58_MULTIBASE_PREFIX as char).unwrap_or(input);
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let base = BASE58_ALPHABET.len() as u128;
    let mut numeric = 0_u128;
    for character in value.chars() {
        let Some(index) = BASE58_ALPHABET.iter().position(|&byte| char::from(byte) == character) else {
            return Err(WireError::UnsupportedTag {
                tag: "invalid-base58".to_owned(),
            });
        };
        numeric = numeric.checked_mul(base).and_then(|n| n.checked_add(index as u128)).ok_or_else(|| {
            WireError::UnsupportedTag {
                tag: "base58-overflow".to_owned(),
            }
        })?;
    }
    let leading_ones = value.chars().take_while(|&character| character == '1').count();
    let mut bytes = Vec::with_capacity(value.len());
    while numeric > 0 {
        bytes.push((numeric % 256) as u8);
        numeric /= 256;
    }
    bytes.reverse();
    for _ in 0..leading_ones {
        bytes.insert(0, 0);
    }
    Ok(bytes)
}

/// Validate an external base58btc CID `1` string and return its bytes.
///
/// The bytes must be version `1`, DAG-CBOR codec, SHA-256 multihash with a
/// 32-byte digest.
///
/// # Errors
///
/// Returns [`WireError`] when the string or structure is invalid.
pub fn cid_validate_string(cid: &str) -> Result<Vec<u8>, WireError> {
    let bytes = base58btc_decode(cid)?;
    validate_cid_bytes(&bytes)?;
    Ok(bytes)
}

/// Validate CIDv1 DAG-CBOR SHA-256 structure bytes.
///
/// # Errors
///
/// Returns [`WireError`] for wrong version, codec, multihash, digest
/// length, or truncated payload.
pub fn validate_cid_bytes(bytes: &[u8]) -> Result<(), WireError> {
    if bytes.len() < 4 + SHA256_DIGEST_LEN {
        return Err(WireError::UnsupportedTag {
            tag: "cid-too-short".to_owned(),
        });
    }
    if bytes[0] != CIDV1_VERSION_BYTE {
        return Err(WireError::UnsupportedTag {
            tag: "cid-wrong-version".to_owned(),
        });
    }
    if bytes[1] != MULTICODEC_DAG_CBOR {
        return Err(WireError::UnsupportedTag {
            tag: "cid-wrong-codec".to_owned(),
        });
    }
    if bytes[2] != MULTIHASH_SHA2_256 {
        return Err(WireError::UnsupportedTag {
            tag: "cid-wrong-multihash".to_owned(),
        });
    }
    if bytes[3] != SHA256_DIGEST_LEN as u8 {
        return Err(WireError::UnsupportedTag {
            tag: "cid-wrong-digest-len".to_owned(),
        });
    }
    Ok(())
}

/// Parse the canonical envelope `[ sig, payload ]` and return the signature
/// plus a typed payload map with field admission applied.
///
/// # Errors
///
/// Returns [`WireError`] on decode, admission, kind, or limit failure.
pub fn parse_envelope(
    envelope: &[u8],
    expected_kind: SpecKind,
) -> Result<([u8; ED25519_SIGNATURE_LEN], Vec<(String, Cbor)>), WireError> {
    if envelope.len() > MAX_WIRE_INPUT_BYTES {
        return Err(WireError::UnsupportedTag {
            tag: "input-too-large".to_owned(),
        });
    }
    let mut decoder = StrictDecoder {
        input: envelope,
        offset: 0,
        depth: 0,
    };
    let value = decoder.read_value()?;
    if decoder.offset != envelope.len() {
        return Err(WireError::UnsupportedTag {
            tag: "trailing-bytes".to_owned(),
        });
    }
    let Cbor::Array(items) = value else {
        return Err(WireError::UnsupportedTag {
            tag: "envelope-array".to_owned(),
        });
    };
    if items.len() != 2 {
        return Err(WireError::UnsupportedTag {
            tag: "envelope-arity".to_owned(),
        });
    }
    let Cbor::Bytes(signature_bytes) = &items[0] else {
        return Err(WireError::UnsupportedTag {
            tag: "signature-not-bytes".to_owned(),
        });
    };
    if signature_bytes.len() != ED25519_SIGNATURE_LEN {
        return Err(WireError::UnsupportedTag {
            tag: "signature-length".to_owned(),
        });
    }
    let mut signature = [0_u8; ED25519_SIGNATURE_LEN];
    signature.copy_from_slice(signature_bytes);
    let Cbor::Bytes(payload_bytes) = &items[1] else {
        return Err(WireError::UnsupportedTag {
            tag: "payload-not-bytes".to_owned(),
        });
    };
    let payload = decode_payload_bytes_to_values(payload_bytes)?;
    let kind = payload_kind(&payload)?;
    if kind != expected_kind {
        return Err(WireError::WrongKind {
            expected: expected_kind,
            actual: kind,
        });
    }
    admit_payload_fields(&payload, expected_kind)?;
    Ok((signature, payload))
}

fn payload_kind(payload: &[(String, Cbor)]) -> Result<SpecKind, WireError> {
    let tag =
        payload
            .iter()
            .find(|(key, _)| key == core::FIELD_TAG_ANNOTATION)
            .ok_or_else(|| WireError::MissingField {
                field: core::FIELD_TAG_ANNOTATION.to_owned(),
            })?;
    let Cbor::Text(tag_text) = &tag.1 else {
        return Err(WireError::UnsupportedTag {
            tag: "tag-not-text".to_owned(),
        });
    };
    SpecKind::from_tag(tag_text).ok_or_else(|| WireError::UnsupportedTag { tag: tag_text.clone() })
}

fn admit_payload_fields(payload: &[(String, Cbor)], kind: SpecKind) -> Result<(), WireError> {
    let mut admission = FieldAdmission::for_kind(kind);
    for (key, value) in payload {
        let byte_len = match value {
            Cbor::Bytes(bytes) => bytes.len(),
            Cbor::Text(text) => text.len(),
            other => other.cbor_len(),
        };
        admission.observe(key, byte_len)?;
    }
    admission.admit()?;
    validate_field_value_kinds(payload, kind)
}

fn validate_field_value_kinds(payload: &[(String, Cbor)], kind: SpecKind) -> Result<(), WireError> {
    for (key, value) in payload {
        let expected_text = matches!(value, Cbor::Text(_));
        let matches = match key.as_str() {
            core::FIELD_ISSUER
            | core::FIELD_AUDIENCE
            | core::FIELD_SUBJECT
            | core::FIELD_COMMAND
            | core::FIELD_TAG_ANNOTATION
            | core::FIELD_VERSION => expected_text,
            core::FIELD_EXPIRES | core::FIELD_NOT_BEFORE => matches!(value, Cbor::UInt(_) | Cbor::NegInt(_)),
            core::FIELD_NONCE => matches!(value, Cbor::Bytes(_)),
            core::FIELD_PROOFS => matches!(value, Cbor::Array(_)),
            core::FIELD_ARGS | core::FIELD_POLICY | core::FIELD_FACTS => true,
            _ => false,
        };
        if !matches {
            return Err(WireError::WrongType { field: key.clone() });
        }
    }
    let _ = kind;
    Ok(())
}

/// Canonicalize a decoded payload map back to its canonical bytes.
#[must_use]
pub fn canonical_payload_bytes(payload: &[(String, Cbor)]) -> Vec<u8> {
    encode_payload_fields(payload.to_vec())
}

/// Build a canonical signed envelope from a payload map and an Ed25519
/// secret key.
#[must_use]
pub fn sign_payload(payload: &[(String, Cbor)], signing_key: &ed25519_dalek::SigningKey) -> Vec<u8> {
    let payload_bytes = canonical_payload_bytes(payload);
    use ed25519_dalek::Signer as _;
    let signature = signing_key.sign(&payload_bytes).to_bytes();
    encode_value(&Cbor::Array(vec![Cbor::Bytes(signature.to_vec()), Cbor::Bytes(payload_bytes)]))
}

/// Verify an envelope against a public key, returning the payload kind.
///
/// # Errors
///
/// Returns [`WireError`] on decode, kind, admission, or signature failure.
pub fn verify_envelope(envelope: &[u8], public_key: &[u8]) -> Result<SpecKind, WireError> {
    if public_key.len() != ED25519_PUBLIC_KEY_LEN {
        return Err(WireError::UnsupportedTag {
            tag: "public-key-length".to_owned(),
        });
    }
    let mut decoder = StrictDecoder {
        input: envelope,
        offset: 0,
        depth: 0,
    };
    let value = decoder.read_value()?;
    if decoder.offset != envelope.len() {
        return Err(WireError::UnsupportedTag {
            tag: "trailing-bytes".to_owned(),
        });
    }
    let Cbor::Array(items) = value else {
        return Err(WireError::UnsupportedTag {
            tag: "envelope-array".to_owned(),
        });
    };
    if items.len() != 2 {
        return Err(WireError::UnsupportedTag {
            tag: "envelope-arity".to_owned(),
        });
    }
    let Cbor::Bytes(signature_bytes) = &items[0] else {
        return Err(WireError::UnsupportedTag {
            tag: "signature-not-bytes".to_owned(),
        });
    };
    let Cbor::Bytes(payload_bytes) = &items[1] else {
        return Err(WireError::UnsupportedTag {
            tag: "payload-not-bytes".to_owned(),
        });
    };
    let signature = ed25519_dalek::Signature::from_slice(signature_bytes).map_err(|_| WireError::UnsupportedTag {
        tag: "bad-signature-bytes".to_owned(),
    })?;
    let verification_key =
        ed25519_dalek::VerifyingKey::from_bytes(public_key.try_into().map_err(|_| WireError::UnsupportedTag {
            tag: "public-key-length".to_owned(),
        })?)
        .map_err(|_| WireError::UnsupportedTag {
            tag: "public-key-decode".to_owned(),
        })?;
    use ed25519_dalek::Verifier as _;
    verification_key.verify(payload_bytes, &signature).map_err(|_| WireError::UnsupportedTag {
        tag: "signature-invalid".to_owned(),
    })?;
    let fields = decode_payload_bytes_to_values(payload_bytes)?;
    payload_kind(&fields)
}

/// A caller-owned signing port. The shell never generates, caches, or
/// persists keys.
pub trait V1Signer {
    /// Sign `message` bytes with the caller-supplied Ed25519 key.
    ///
    /// # Errors
    ///
    /// Returns a bounded message when the signer backend fails.
    fn ed25519_sign(&self, message: &[u8]) -> Result<[u8; ED25519_SIGNATURE_LEN], String>;
}

/// A caller-owned DID key resolver port. The shell performs no DID
/// resolution or key discovery.
pub trait DidKeyResolver {
    /// Resolve the 32-byte Ed25519 verification key for a `did:key`
    /// identifier.
    ///
    /// # Errors
    ///
    /// Returns a bounded message when the resolver cannot answer.
    fn resolve_did_key(&self, did: &str) -> Result<[u8; ED25519_PUBLIC_KEY_LEN], String>;
}

/// Adapter from an Ed25519 public key to a resolver answer for the issuer
/// DID derived from it.
#[must_use]
pub fn did_key_from_public_key(public_key: &[u8]) -> Option<String> {
    let key = ed25519_dalek::VerifyingKey::from_bytes(public_key.try_into().ok()?).ok()?;
    Some(format!("did:key:{}", base58btc_encode(&encode_did_key_prefix(&key))))
}

fn encode_did_key_prefix(key: &ed25519_dalek::VerifyingKey) -> Vec<u8> {
    // did:key multibase encoding for Ed25519 public keys: multicodec
    // `ed25519-pub` (0xed 0x01) + the 32-byte key, base58btc with `z`.
    let mut bytes = vec![0xED, 0x01];
    bytes.extend_from_slice(key.as_bytes());
    bytes
}

/// A concrete no-ambient `V1Signer` backed by an Ed25519 signing key.
pub struct StaticEd25519Signer(pub ed25519_dalek::SigningKey);

impl V1Signer for StaticEd25519Signer {
    fn ed25519_sign(&self, message: &[u8]) -> Result<[u8; ED25519_SIGNATURE_LEN], String> {
        use ed25519_dalek::Signer as _;
        Ok(self.0.sign(message).to_bytes())
    }
}

/// A concrete no-ambient `DidKeyResolver` over an explicit key map.
pub struct StaticDidKeyResolver {
    keys: std::collections::BTreeMap<String, [u8; ED25519_PUBLIC_KEY_LEN]>,
}

impl StaticDidKeyResolver {
    /// Build a resolver from an explicit DID-to-key map.
    #[must_use]
    pub const fn new(keys: std::collections::BTreeMap<String, [u8; ED25519_PUBLIC_KEY_LEN]>) -> Self {
        Self { keys }
    }
}

impl DidKeyResolver for StaticDidKeyResolver {
    fn resolve_did_key(&self, did: &str) -> Result<[u8; ED25519_PUBLIC_KEY_LEN], String> {
        self.keys.get(did).copied().ok_or_else(|| format!("unresolved DID {did}"))
    }
}

/// Extract a bounded wire payload map from JSON fixture metadata.
///
/// This helper projects the pinned fixture `envelope.payload` objects onto
/// the profile types. It is a shell projection, not a conformance claim.
///
/// # Panics
///
/// Panics when a fixture timestamp is negative; the pinned fixtures use
/// non-negative Unix seconds.
#[must_use]
pub fn payload_from_json_metadata(kind: SpecKind, value: &serde_json::Value) -> Vec<(String, Cbor)> {
    let mut fields: Vec<(String, Cbor)> = Vec::new();
    fields.push((core::FIELD_TAG_ANNOTATION.to_owned(), Cbor::Text(kind.tag().to_owned())));
    fields.push(("version".to_owned(), Cbor::Text(core::WIRE_VERSION.to_owned())));
    for key in [
        core::FIELD_ISSUER,
        core::FIELD_AUDIENCE,
        core::FIELD_SUBJECT,
        core::FIELD_COMMAND,
    ] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str) {
            fields.push((key.to_owned(), Cbor::Text(text.to_owned())));
        }
    }
    if let Some(exp) = value.get(core::FIELD_EXPIRES).and_then(serde_json::Value::as_i64) {
        let positive = u64::try_from(exp).expect("exp is a non-negative Unix timestamp in fixtures");
        fields.push((core::FIELD_EXPIRES.to_owned(), Cbor::UInt(positive)));
    }
    if let Some(nbf) = value.get(core::FIELD_NOT_BEFORE).and_then(serde_json::Value::as_i64) {
        let positive = u64::try_from(nbf).expect("nbf is a non-negative Unix timestamp in fixtures");
        fields.push((core::FIELD_NOT_BEFORE.to_owned(), Cbor::UInt(positive)));
    }
    if let Some(nonce) = value.get(core::FIELD_NONCE).and_then(serde_json::Value::as_str) {
        fields.push((core::FIELD_NONCE.to_owned(), Cbor::Bytes(nonce.as_bytes().to_vec())));
    }
    if let Some(proofs) = value.get(core::FIELD_PROOFS).and_then(serde_json::Value::as_array) {
        let items = proofs
            .iter()
            .filter_map(|proof| proof.as_str())
            .map(|proof| Cbor::Bytes(proof.as_bytes().to_vec()))
            .collect();
        fields.push((core::FIELD_PROOFS.to_owned(), Cbor::Array(items)));
    }
    if let Some(policy) = value.get(core::FIELD_POLICY) {
        fields.push((core::FIELD_POLICY.to_owned(), cbor_from_json(policy)));
    }
    if let Some(args) = value.get(core::FIELD_ARGS) {
        fields.push((core::FIELD_ARGS.to_owned(), cbor_from_json(args)));
    }
    fields
}

fn cbor_from_json(value: &serde_json::Value) -> Cbor {
    match value {
        serde_json::Value::Null => Cbor::Null,
        serde_json::Value::Bool(value) => Cbor::Bool(*value),
        serde_json::Value::Number(number) => number.as_i64().map_or(Cbor::UInt(0), |integer| {
            if integer < 0 {
                Cbor::NegInt(integer)
            } else {
                Cbor::UInt(u64::try_from(integer).expect("non-negative i64 fits u64"))
            }
        }),
        serde_json::Value::String(text) => Cbor::Text(text.clone()),
        serde_json::Value::Array(items) => Cbor::Array(items.iter().map(cbor_from_json).collect()),
        serde_json::Value::Object(map) => {
            Cbor::Map(map.iter().map(|(key, item)| (key.clone(), cbor_from_json(item))).collect())
        }
    }
}

/// Project a JSON value to the bounded wire CBOR value used by the profile
/// payload builders.
#[must_use]
pub fn cbor_from_json_value(value: &serde_json::Value) -> Cbor {
    cbor_from_json(value)
}

/// Serialize a payload field list back to JSON form for evidence rendering.
#[must_use]
pub fn payload_to_json(payload: &[(String, Cbor)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in payload {
        map.insert(key.clone(), cbor_to_json(value));
    }
    serde_json::Value::Object(map)
}

fn cbor_to_json(value: &Cbor) -> serde_json::Value {
    match value {
        Cbor::Null => serde_json::Value::Null,
        Cbor::Bool(value) => serde_json::Value::Bool(*value),
        Cbor::UInt(value) => serde_json::Value::Number(serde_json::Number::from(*value)),
        Cbor::NegInt(value) => serde_json::Value::Number(serde_json::Number::from(*value)),
        Cbor::Bytes(bytes) => serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(bytes)),
        Cbor::Text(text) => serde_json::Value::String(text.clone()),
        Cbor::Array(items) => serde_json::Value::Array(items.iter().map(cbor_to_json).collect()),
        Cbor::Map(items) => {
            let mut map = serde_json::Map::new();
            for (key, item) in items {
                map.insert(key.clone(), cbor_to_json(item));
            }
            serde_json::Value::Object(map)
        }
    }
}
