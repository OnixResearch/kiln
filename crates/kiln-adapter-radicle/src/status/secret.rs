use radicle_crypto::{Seed, SigningKey};
use std::fmt;

use kiln_core::types::{CoreResult, Diagnostic};

/// Resolved signer material from SecretSpec.
///
/// The seed is intentionally private. The `Debug` impl never renders it, so
/// accidental logging cannot leak key bytes.
pub struct ResolvedSigner {
    signing_key: SigningKey,
    public: String,
}

pub const SEED_BYTES: usize = 32;
const HEX_CHARACTERS_PER_BYTE: usize = 2;
const HEX_NIBBLE_BITS: u32 = 4;
const HEX_ALPHA_OFFSET: u8 = 10;
const HEX_SEED_LENGTH: usize = SEED_BYTES * HEX_CHARACTERS_PER_BYTE;

impl fmt::Debug for ResolvedSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedSigner")
            .field("public", &self.public)
            .field("seed", &"[redacted]")
            .finish()
    }
}

impl ResolvedSigner {
    // r[impl kiln.radicle.status_signer.scenario.resolved]
    /// Parse one resolved SecretSpec value as a 64-character hexadecimal
    /// Ed25519 seed and derive the Radicle signer from it.
    pub fn from_resolved_secret(secret: &str) -> CoreResult<Self> {
        let seed = parse_seed(secret)?;
        let signing_key = SigningKey::from_seed(Seed::new(seed));
        let public = {
            use radicle_crypto::signature::Keypair as _;
            signing_key.verifying_key().public_key().to_string()
        };
        Ok(Self {
            public,
            signing_key,
        })
    }

    #[must_use]
    pub const fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public
    }
}

fn parse_seed(secret: &str) -> CoreResult<[u8; SEED_BYTES]> {
    let trimmed = secret.trim();
    if trimmed.len() != HEX_SEED_LENGTH || !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Diagnostic::new(
            "radicle_status_signer_unresolved",
            "radicle_status.signer",
            "SecretSpec value is not a valid 64-character hexadecimal signer seed",
        ));
    }
    let mut seed = [0; SEED_BYTES];
    for (output, encoded) in seed
        .iter_mut()
        .zip(trimmed.as_bytes().chunks_exact(HEX_CHARACTERS_PER_BYTE))
    {
        let high = encoded.first().copied().ok_or_else(seed_format_error)?;
        let low = encoded.get(1).copied().ok_or_else(seed_format_error)?;
        *output = (hex_value(high)? << HEX_NIBBLE_BITS) | hex_value(low)?;
    }
    if seed == [0; SEED_BYTES] {
        return Err(Diagnostic::new(
            "radicle_status_signer_unresolved",
            "radicle_status.signer",
            "SecretSpec signer seed must not be all zeros",
        ));
    }
    Ok(seed)
}

fn seed_format_error() -> Diagnostic {
    Diagnostic::new(
        "radicle_status_signer_unresolved",
        "radicle_status.signer",
        "SecretSpec signer seed had an incomplete encoded byte",
    )
}

fn hex_value(byte: u8) -> CoreResult<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + HEX_ALPHA_OFFSET),
        b'A'..=b'F' => Ok(byte - b'A' + HEX_ALPHA_OFFSET),
        _ => Err(Diagnostic::new(
            "radicle_status_signer_unresolved",
            "radicle_status.signer",
            "SecretSpec signer seed contains a non-hexadecimal character",
        )),
    }
}
