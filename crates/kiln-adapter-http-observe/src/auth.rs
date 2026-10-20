use std::collections::BTreeMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{ObserveError, ObserveResult};

pub const AUTH_SCHEMA: &str = "kiln.observe.http.auth.v1";
pub const AUTH_HEADER_SCHEME: &str = "Kiln-Observe";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthPayload {
    pub schema: String,
    pub method: String,
    pub path: String,
    pub body_blake3: String,
    pub nonce: String,
    pub issued_at: u64,
    pub state_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a [u8],
    pub state_revision: u64,
}

pub fn parse_principal(value: &str) -> ObserveResult<[u8; 32]> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return decode_hex(value);
    }
    if let Some(encoded) = value.strip_prefix("did:key:z") {
        let bytes = base58_decode(encoded)?;
        if bytes.len() == 34 && bytes.get(0..2) == Some([0xed, 0x01].as_slice()) {
            let key = bytes.get(2..).unwrap_or_default();
            return key.try_into().map_err(|_| {
                ObserveError::new("invalid_principal", "did:key Ed25519 key is malformed")
            });
        }
        return Err(ObserveError::new(
            "invalid_principal",
            "did:key is not an Ed25519 multicodec key",
        ));
    }
    Err(ObserveError::new(
        "invalid_principal",
        "principal must be 64 lowercase hex characters or a did:key Ed25519 reference",
    ))
}

// r[impl kiln.observe.http.auth]
pub fn verify_auth_header(
    header: &str,
    principal: &[u8; 32],
    request: &AuthRequest<'_>,
    now_unix: u64,
    max_clock_skew_seconds: u64,
    max_nonce_bytes: usize,
) -> ObserveResult<AuthPayload> {
    let token = header
        .strip_prefix(AUTH_HEADER_SCHEME)
        .and_then(|rest| rest.strip_prefix(' '))
        .ok_or_else(|| ObserveError::new("auth_scheme", "authorization scheme is unsupported"))?;
    let (payload_part, signature_part) = token
        .split_once('.')
        .ok_or_else(|| ObserveError::new("auth_shape", "authorization token is malformed"))?;
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_part)
        .map_err(|_| ObserveError::new("auth_payload", "authorization payload is not base64url"))?;
    let signature_bytes = URL_SAFE_NO_PAD.decode(signature_part).map_err(|_| {
        ObserveError::new("auth_signature", "authorization signature is not base64url")
    })?;
    let payload: AuthPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| ObserveError::new("auth_payload", "authorization payload is malformed"))?;
    if payload.schema != AUTH_SCHEMA
        || payload.method != request.method
        || payload.path != request.path
        || payload.body_blake3 != body_digest(request.body)
        || payload.state_revision != request.state_revision
    {
        return Err(ObserveError::new(
            "auth_binding",
            "authorization payload does not bind this request",
        ));
    }
    if payload.nonce.is_empty() || payload.nonce.len() > max_nonce_bytes {
        return Err(ObserveError::new(
            "auth_nonce",
            "authorization nonce is empty or exceeds its bound",
        ));
    }
    let skew = now_unix.abs_diff(payload.issued_at);
    if skew > max_clock_skew_seconds {
        return Err(ObserveError::new(
            "auth_clock",
            "authorization issue time is outside the admitted clock skew",
        ));
    }
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| ObserveError::new("auth_signature", "authorization signature is malformed"))?;
    let key = VerifyingKey::from_bytes(principal)
        .map_err(|_| ObserveError::new("invalid_principal", "Ed25519 principal is invalid"))?;
    key.verify(&payload_bytes, &signature)
        .map_err(|_| ObserveError::new("auth_signature", "authorization signature is invalid"))?;
    Ok(payload)
}

#[derive(Debug)]
pub struct ReplayState {
    entries: BTreeMap<Vec<u8>, u64>,
    window_seconds: u64,
    max_entries: usize,
}

impl ReplayState {
    #[must_use]
    pub fn new(window_seconds: u64, max_entries: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            window_seconds,
            max_entries,
        }
    }

    pub fn admit(&mut self, payload: &AuthPayload, now_unix: u64) -> ObserveResult<()> {
        self.entries
            .retain(|_, issued_at| now_unix.saturating_sub(*issued_at) <= self.window_seconds);
        let nonce = payload.nonce.as_bytes().to_vec();
        if self.entries.contains_key(&nonce) {
            return Err(ObserveError::new(
                "auth_replay",
                "authorization nonce was already admitted",
            ));
        }
        if self.entries.len() >= self.max_entries {
            return Err(ObserveError::new(
                "auth_replay_bound",
                "authorization replay cache is full",
            ));
        }
        self.entries.insert(nonce, payload.issued_at);
        Ok(())
    }
}

#[must_use]
pub fn body_digest(body: &[u8]) -> String {
    blake3::hash(body).to_hex().to_string()
}

fn decode_hex(value: &str) -> ObserveResult<[u8; 32]> {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return Err(ObserveError::new(
            "invalid_principal",
            "hex principal must contain 64 characters",
        ));
    }
    let mut output = [0_u8; 32];
    let mut index = 0;
    while index < output.len() {
        let high = hex_value(*bytes.get(index * 2).unwrap_or(&b'0'))?;
        let low = hex_value(*bytes.get(index * 2 + 1).unwrap_or(&b'0'))?;
        let slot = output.get_mut(index).ok_or_else(|| {
            ObserveError::new("invalid_principal", "hex principal range is invalid")
        })?;
        *slot = (high << 4) | low;
        index += 1;
    }
    Ok(output)
}

fn hex_value(byte: u8) -> ObserveResult<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ObserveError::new(
            "invalid_principal",
            "principal contains a non-hexadecimal character",
        )),
    }
}

fn base58_decode(value: &str) -> ObserveResult<Vec<u8>> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut output: Vec<u8> = Vec::new();
    for character in value.bytes() {
        let digit = ALPHABET
            .iter()
            .position(|candidate| *candidate == character)
            .ok_or_else(|| {
                ObserveError::new("invalid_principal", "did:key contains invalid base58")
            })? as u32;
        let mut carry = digit;
        for byte in output.iter_mut().rev() {
            let value = u32::from(*byte) * 58 + carry;
            *byte = (value & 0xff) as u8;
            carry = value >> 8;
        }
        while carry > 0 {
            output.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    for character in value.bytes() {
        if character == b'1' {
            output.insert(0, 0);
        } else {
            break;
        }
    }
    Ok(output)
}
