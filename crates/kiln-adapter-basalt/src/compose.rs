//! Composition helpers for the verified profile: static keys, the bounded
//! proof store and holder-grant selection.

use ucan::{
    CompactToken, Ed25519PublicKey, Ed25519VerificationKey, IssuerDid, KeyResolutionContext,
    NoRevocations, ProofCollection, VerificationLimits, VerificationTime,
};

/// Ed25519 public-key width.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;

/// Build the static key resolver from operator-admitted `(did, public key)`
/// pairs. Each key must encode to its DID.
///
/// # Errors
/// Returns a message for an invalid DID or a key that does not match it.
pub fn static_keys(
    keys: &[(String, [u8; ED25519_PUBLIC_KEY_BYTES])],
) -> Result<KeyResolutionContext, String> {
    let mut resolved = Vec::with_capacity(keys.len());
    for (did, public_key) in keys {
        let issuer = IssuerDid::new(did.clone()).map_err(|_| "key DID is invalid".to_string())?;
        resolved.push(
            Ed25519VerificationKey::new(issuer, Ed25519PublicKey::from_bytes(*public_key))
                .map_err(|_| "key does not encode to its DID".to_string())?,
        );
    }
    Ok(KeyResolutionContext::new(resolved))
}

/// Build the proof store from bounded compact tokens.
///
/// # Errors
/// Returns a message for a token that is not a compact token.
pub fn proof_store(tokens: &[String]) -> Result<ProofCollection, String> {
    let mut parsed = Vec::with_capacity(tokens.len());
    for token in tokens {
        parsed.push(CompactToken::new(token).map_err(|_| "proof is not a compact token")?);
    }
    Ok(ProofCollection::from_tokens(parsed))
}

/// Select the one implementation grant addressed to `holder_did` among `tokens`.
///
/// Selection verifies signatures, proof chains and time at `now_unix_s` so a
/// stale or foreign token is never chosen; revocation and replay are enforced
/// at admission, not here.
///
/// # Errors
/// Returns a message when no token or more than one token is addressed to the
/// holder.
pub fn select_holder_grant(
    tokens: &[String],
    holder_did: &str,
    keys: &KeyResolutionContext,
    proofs: &ProofCollection,
    now_unix_s: u64,
    limits: VerificationLimits,
) -> Result<String, String> {
    select_family_grant(
        tokens,
        holder_did,
        keys,
        proofs,
        now_unix_s,
        limits,
        |resource| resource.starts_with("kiln://implementation/") || resource == "kiln://",
    )
}

/// Select the one `kiln://source/v1/` grant addressed to `holder_did` among `tokens`.
///
/// # Errors
/// Returns a message when no source token or more than one source token is
/// addressed to the holder.
pub fn select_source_holder_grant(
    tokens: &[String],
    holder_did: &str,
    keys: &KeyResolutionContext,
    proofs: &ProofCollection,
    now_unix_s: u64,
    limits: VerificationLimits,
) -> Result<String, String> {
    select_family_grant(
        tokens,
        holder_did,
        keys,
        proofs,
        now_unix_s,
        limits,
        |resource| resource.starts_with("kiln://source/v1/"),
    )
}

fn select_family_grant(
    tokens: &[String],
    holder_did: &str,
    keys: &KeyResolutionContext,
    proofs: &ProofCollection,
    now_unix_s: u64,
    limits: VerificationLimits,
    matches_family: impl Fn(&str) -> bool,
) -> Result<String, String> {
    let mut selected = None;
    for token in tokens {
        let Ok(verified) = ucan::verify_compact_token_str_with_resolvers_and_revocations(
            token,
            VerificationTime::from_unix_seconds(now_unix_s),
            keys,
            proofs,
            &NoRevocations,
            limits,
        ) else {
            continue;
        };
        if verified.audience().as_str() != holder_did {
            continue;
        }
        let has_family = verified
            .effective_delegation()
            .capabilities()
            .as_slice()
            .iter()
            .any(|cap| matches_family(&cap.resource));
        if !has_family {
            continue;
        }
        if selected.replace(token.clone()).is_some() {
            return Err("more than one current grant is addressed to one holder".into());
        }
    }
    selected.ok_or_else(|| "no current grant is addressed to the holder".into())
}
