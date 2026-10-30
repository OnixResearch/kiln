//! AT Protocol service-auth verification and Tangled webhook signatures.
//!
//! Tokens are compact JWS (`header.claims.signature`, base64url without
//! padding). Knots sign with secp256k1 (`ES256K`); PDS-minted user tokens and
//! spindles may use P-256 (`ES256`). The signature is the 64-byte `r || s`
//! encoding over the exact `header.claims` bytes, and must be low-S.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::error::{Rejection, TangledResult};
use crate::identity::valid_did;

/// Tokens longer than this are refused before decoding.
pub const MAXIMUM_TOKEN_BYTES: usize = 8192;
/// Knots use 16-byte hex nonces; Tangled caps nonces at 256 bytes.
pub const MAXIMUM_JTI_BYTES: usize = 256;
/// DID documents with more verification methods are refused.
pub const MAXIMUM_VERIFICATION_METHODS: usize = 32;
/// HMAC-SHA256 output in bytes.
const HMAC_BYTES: usize = 32;
const COMPRESSED_POINT_BYTES: usize = 33;
/// Multicodec varint prefixes (`secp256k1-pub` 0xe7, `p256-pub` 0x1200).
const SECP256K1_PREFIX: [u8; 2] = [0xe7, 0x01];
const P256_PREFIX: [u8; 2] = [0x80, 0x24];
const SIGNATURE_BYTES: usize = 64;

/// A verifying key taken from an issuer's DID document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssuerKey {
    Secp256k1(k256::ecdsa::VerifyingKey),
    P256(p256::ecdsa::VerifyingKey),
}

/// What the verifier requires of a token.
#[derive(Clone, Copy, Debug)]
pub struct Expectation<'a> {
    pub audience: &'a str,
    pub method: &'a str,
    pub now_unix: i64,
    pub maximum_lifetime_seconds: i64,
    pub clock_skew_seconds: i64,
}

/// Claims of a verified token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedClaims {
    pub issuer: String,
    pub jti: String,
    pub expires_unix: i64,
}

/// A token split and decoded but not yet verified. Only `issuer` may be used
/// before verification, to select the key to resolve.
#[derive(Clone, Debug)]
pub struct ParsedToken {
    pub issuer: String,
    algorithm: Algorithm,
    signing_input: String,
    signature: [u8; SIGNATURE_BYTES],
    claims: Claims,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Algorithm {
    Es256k,
    Es256,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    #[serde(default)]
    typ: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Claims {
    iss: String,
    aud: String,
    exp: i64,
    #[serde(default)]
    iat: Option<i64>,
    #[serde(default)]
    lxm: Option<String>,
    #[serde(default)]
    jti: Option<String>,
}

fn malformed() -> Rejection {
    Rejection::authentication("tangled_auth_malformed", "Service-auth token is malformed")
}

/// Extract the token from an `Authorization: Bearer` header value.
pub fn bearer(header: Option<&str>) -> TangledResult<&str> {
    header
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or(Rejection::authentication(
            "tangled_auth_missing",
            "Service-auth bearer token is required",
        ))
}

// r[impl kiln.tangled.auth]
pub fn parse_token(token: &str) -> TangledResult<ParsedToken> {
    if token.len() > MAXIMUM_TOKEN_BYTES {
        return Err(malformed());
    }
    let mut parts = token.split('.');
    let (Some(header), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(malformed());
    };
    if header.is_empty() || claims.is_empty() || signature.is_empty() {
        return Err(malformed());
    }
    let decode = |segment: &str| URL_SAFE_NO_PAD.decode(segment).map_err(|_| malformed());
    let header_value: Header = serde_json::from_slice(&decode(header)?).map_err(|_| malformed())?;
    if header_value.typ.as_deref().is_some_and(|typ| typ != "JWT") {
        return Err(Rejection::authentication(
            "tangled_auth_type",
            "Service-auth token type is not JWT",
        ));
    }
    let algorithm = match header_value.alg.as_str() {
        "ES256K" => Algorithm::Es256k,
        "ES256" => Algorithm::Es256,
        _ => {
            return Err(Rejection::authentication(
                "tangled_auth_algorithm",
                "Service-auth algorithm is not ES256K or ES256",
            ));
        }
    };
    let claims_value: Claims = serde_json::from_slice(&decode(claims)?).map_err(|_| malformed())?;
    if !valid_did(&claims_value.iss) {
        return Err(malformed());
    }
    let signature = <[u8; SIGNATURE_BYTES]>::try_from(decode(signature)?.as_slice())
        .map_err(|_| malformed())?;
    Ok(ParsedToken {
        issuer: claims_value.iss.clone(),
        algorithm,
        signing_input: format!("{header}.{claims}"),
        signature,
        claims: claims_value,
    })
}

fn signature_failure() -> Rejection {
    Rejection::authentication(
        "tangled_auth_signature",
        "Service-auth signature does not verify against the issuer key",
    )
}

// r[impl kiln.tangled.auth]
pub fn verify_token(
    token: &ParsedToken,
    key: &IssuerKey,
    expectation: &Expectation<'_>,
) -> TangledResult<VerifiedClaims> {
    verify_signature(token, key)?;
    let claims = &token.claims;
    if claims.aud != expectation.audience {
        return Err(Rejection::authentication(
            "tangled_auth_audience",
            "Service-auth token is addressed to another service",
        ));
    }
    if claims.lxm.as_deref() != Some(expectation.method) {
        return Err(Rejection::authentication(
            "tangled_auth_method",
            "Service-auth token is not bound to this method",
        ));
    }
    let now = expectation.now_unix;
    let skew = expectation.clock_skew_seconds;
    if claims.exp.saturating_add(skew) <= now {
        return Err(Rejection::authentication(
            "tangled_auth_expired",
            "Service-auth token has expired",
        ));
    }
    let issued = claims.iat.unwrap_or(now);
    if issued > now.saturating_add(skew)
        || claims.exp < issued
        || claims.exp.saturating_sub(now.min(issued)) > expectation.maximum_lifetime_seconds
    {
        return Err(Rejection::authentication(
            "tangled_auth_lifetime",
            "Service-auth token lifetime is outside its bound",
        ));
    }
    let jti = claims
        .jti
        .as_deref()
        .filter(|jti| !jti.is_empty() && jti.len() <= MAXIMUM_JTI_BYTES)
        .ok_or(Rejection::authentication(
            "tangled_auth_jti",
            "Service-auth token has no bounded jti for replay protection",
        ))?;
    Ok(VerifiedClaims {
        issuer: claims.iss.clone(),
        jti: jti.to_string(),
        expires_unix: claims.exp,
    })
}

fn verify_signature(token: &ParsedToken, key: &IssuerKey) -> TangledResult<()> {
    use k256::ecdsa::signature::Verifier as _;
    let message = token.signing_input.as_bytes();
    match (token.algorithm, key) {
        (Algorithm::Es256k, IssuerKey::Secp256k1(key)) => {
            let signature = k256::ecdsa::Signature::from_slice(&token.signature)
                .map_err(|_| signature_failure())?;
            if signature.normalize_s().is_some() {
                return Err(signature_failure());
            }
            key.verify(message, &signature)
                .map_err(|_| signature_failure())
        }
        (Algorithm::Es256, IssuerKey::P256(key)) => {
            let signature = p256::ecdsa::Signature::from_slice(&token.signature)
                .map_err(|_| signature_failure())?;
            if signature.normalize_s().is_some() {
                return Err(signature_failure());
            }
            key.verify(message, &signature)
                .map_err(|_| signature_failure())
        }
        _ => Err(Rejection::authentication(
            "tangled_auth_key_algorithm",
            "Service-auth algorithm does not match the issuer key type",
        )),
    }
}

#[derive(Deserialize)]
struct DidDocument {
    id: String,
    #[serde(rename = "verificationMethod", default)]
    verification_method: Vec<VerificationMethod>,
}

#[derive(Deserialize)]
struct VerificationMethod {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "publicKeyMultibase", default)]
    public_key_multibase: Option<String>,
}

/// Errors of DID-document parsing are domain failures of the document, not of
/// the transport that fetched it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentError {
    Malformed,
    IdentityMismatch,
    MissingKey,
    UnsupportedKey,
}

/// The `#atproto` Multikey of `did`'s DID document.
pub fn atproto_key(
    did: &str,
    document: &[u8],
    maximum_bytes: usize,
) -> Result<IssuerKey, DocumentError> {
    if document.len() > maximum_bytes {
        return Err(DocumentError::Malformed);
    }
    let document: DidDocument =
        serde_json::from_slice(document).map_err(|_| DocumentError::Malformed)?;
    if document.id != did {
        return Err(DocumentError::IdentityMismatch);
    }
    if document.verification_method.len() > MAXIMUM_VERIFICATION_METHODS {
        return Err(DocumentError::Malformed);
    }
    let qualified = format!("{did}#atproto");
    let method = document
        .verification_method
        .iter()
        .find(|method| method.id == "#atproto" || method.id == qualified)
        .ok_or(DocumentError::MissingKey)?;
    if method.kind != "Multikey" {
        return Err(DocumentError::UnsupportedKey);
    }
    let encoded = method
        .public_key_multibase
        .as_deref()
        .and_then(|value| value.strip_prefix('z'))
        .ok_or(DocumentError::UnsupportedKey)?;
    let bytes = bs58::decode(encoded)
        .into_vec()
        .map_err(|_| DocumentError::Malformed)?;
    parse_multikey(&bytes)
}

fn parse_multikey(bytes: &[u8]) -> Result<IssuerKey, DocumentError> {
    let (prefix, point) = bytes.split_at_checked(2).ok_or(DocumentError::Malformed)?;
    if point.len() != COMPRESSED_POINT_BYTES {
        return Err(DocumentError::UnsupportedKey);
    }
    if prefix == SECP256K1_PREFIX {
        k256::ecdsa::VerifyingKey::from_sec1_bytes(point)
            .map(IssuerKey::Secp256k1)
            .map_err(|_| DocumentError::Malformed)
    } else if prefix == P256_PREFIX {
        p256::ecdsa::VerifyingKey::from_sec1_bytes(point)
            .map(IssuerKey::P256)
            .map_err(|_| DocumentError::Malformed)
    } else {
        Err(DocumentError::UnsupportedKey)
    }
}

/// Multibase (`z` base58btc) Multikey of a compressed public key; used by
/// fixtures and operators publishing DID documents.
#[must_use]
pub fn multikey(key: &IssuerKey) -> String {
    let mut bytes = Vec::with_capacity(2 + COMPRESSED_POINT_BYTES);
    match key {
        IssuerKey::Secp256k1(key) => {
            bytes.extend_from_slice(&SECP256K1_PREFIX);
            bytes.extend_from_slice(key.to_encoded_point(true).as_bytes());
        }
        IssuerKey::P256(key) => {
            bytes.extend_from_slice(&P256_PREFIX);
            bytes.extend_from_slice(key.to_encoded_point(true).as_bytes());
        }
    }
    format!("z{}", bs58::encode(bytes).into_string())
}

/// `X-Tangled-Signature-256` value for `body` (Tangled webhook format).
#[must_use]
pub fn webhook_signature(body: &[u8], secret: &[u8]) -> Option<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    Some(format!(
        "sha256={}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

// r[impl kiln.tangled.auth]
/// Constant-time verification of a Tangled webhook signature header.
pub fn verify_webhook_signature(header: &str, body: &[u8], secret: &[u8]) -> TangledResult<()> {
    let failure = || {
        Rejection::authentication(
            "tangled_webhook_signature",
            "Webhook signature does not verify",
        )
    };
    if secret.is_empty() {
        return Err(failure());
    }
    let encoded = header
        .strip_prefix("sha256=")
        .filter(|value| value.len() == HMAC_BYTES * 2)
        .ok_or_else(failure)?;
    let mut expected = [0_u8; HMAC_BYTES];
    for (pair, output) in encoded.as_bytes().chunks_exact(2).zip(&mut expected) {
        let [high, low] = pair else {
            return Err(failure());
        };
        *output =
            hex_digit(*high).ok_or_else(failure)? * 16 + hex_digit(*low).ok_or_else(failure)?;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| failure())?;
    mac.update(body);
    mac.verify_slice(&expected).map_err(|_| failure())
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
