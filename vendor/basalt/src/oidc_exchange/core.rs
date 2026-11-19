use base64::Engine as _;
use ring::signature;
use serde::Deserialize;

use super::model::{
    ExchangeRequest, IssuancePlan, OidcPolicy, PublicJwk, RejectReason, ReplayKey, VerifiedSubject,
};

const REQUIRED_TOKEN_TYPE: &str = "JWT";
const REQUIRED_ALGORITHM: &str = "RS256";
const MAX_KEY_ID_BYTES: usize = 512;
const MAX_SUBJECT_BYTES: usize = 2048;
const GIT_COMMIT_HEX_BYTES: usize = 40;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenHeader {
    alg: String,
    #[serde(default)]
    typ: Option<String>,
    #[serde(default)]
    kid: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OidcClaims {
    iss: String,
    sub: String,
    aud: AudienceClaim,
    exp: u64,
    nbf: u64,
    iat: u64,
    jti: String,
    repository_owner: String,
    repository: String,
    job_workflow_ref: String,
    sha: String,
    #[serde(rename = "ref")]
    git_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum AudienceClaim {
    One(String),
    Many(Vec<String>),
}

impl AudienceClaim {
    fn contains(&self, expected: &str) -> bool {
        match self {
            Self::One(value) => value == expected,
            Self::Many(values) => values.iter().any(|value| value == expected),
        }
    }

    fn is_bounded(&self) -> bool {
        match self {
            Self::One(value) => bounded_claim(value),
            Self::Many(values) => {
                !values.is_empty()
                    && values.len() <= super::model::policy_item_limit()
                    && values.iter().all(|value| bounded_claim(value))
            }
        }
    }
}

/// Inspect the untrusted header only to select a candidate public key.
///
/// Signature and claim validation must still follow with [`verify_and_plan`].
///
/// # Errors
///
/// Returns a stable rejection for excessive, malformed, unsupported, or keyless headers.
pub fn inspect_token_key_id(token: &str, maximum_bytes: u64) -> Result<String, RejectReason> {
    enforce_token_bound(token, maximum_bytes)?;
    let header = parse_header(token)?;
    validate_header(&header)?;
    header.kid.ok_or(RejectReason::Key)
}

/// Verify one token and attenuate one exact Mantle capability request.
///
/// This function performs no network, clock, cache, replay, storage, signing, or
/// issuance effects. `now_seconds` and `jwk` are explicit caller observations.
///
/// # Errors
///
/// Returns one stable rejection and never returns a partial issuance plan.
pub fn verify_and_plan(
    token: &str,
    jwk: &PublicJwk,
    now_seconds: u64,
    policy: &OidcPolicy,
    request: &ExchangeRequest,
) -> Result<IssuancePlan, RejectReason> {
    enforce_token_bound(token, policy.network.token_bytes)?;
    let header = parse_header(token)?;
    validate_header(&header)?;
    if header.kid.as_deref() != Some(jwk.key_id.as_str()) {
        return Err(RejectReason::Key);
    }
    let claims = verify_rs256(token, jwk)?;
    validate_claim_shapes(&claims)?;
    validate_identity_claims(&claims, now_seconds, policy)?;
    attenuate(&claims, jwk, now_seconds, policy, request)
}

fn enforce_token_bound(token: &str, maximum_bytes: u64) -> Result<(), RejectReason> {
    let length = u64::try_from(token.len()).map_err(|_error| RejectReason::TokenTooLarge)?;
    if token.is_empty() || length > maximum_bytes {
        return Err(RejectReason::TokenTooLarge);
    }
    Ok(())
}

/// One compact JWS split into its exact three non-empty segments.
fn segments(token: &str) -> Result<(&str, &str, &str), RejectReason> {
    let mut parts = token.split('.');
    let (Some(header), Some(claims), Some(signature)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(RejectReason::MalformedToken);
    };
    if parts.next().is_some() || header.is_empty() || claims.is_empty() || signature.is_empty() {
        return Err(RejectReason::MalformedToken);
    }
    Ok((header, claims, signature))
}

fn decode_segment(segment: &str) -> Result<Vec<u8>, RejectReason> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|_error| RejectReason::MalformedToken)
}

/// The header is untrusted: any field outside the reviewed set rejects the token.
fn parse_header(token: &str) -> Result<TokenHeader, RejectReason> {
    let (header, _, _) = segments(token)?;
    serde_json::from_slice(&decode_segment(header)?).map_err(|_error| RejectReason::MalformedToken)
}

fn validate_header(header: &TokenHeader) -> Result<(), RejectReason> {
    if header.alg != REQUIRED_ALGORITHM {
        return Err(RejectReason::Algorithm);
    }
    if header.typ.as_deref() != Some(REQUIRED_TOKEN_TYPE) {
        return Err(RejectReason::TokenType);
    }
    let key_id = header.kid.as_deref().ok_or(RejectReason::Key)?;
    if key_id.is_empty() || key_id.len() > MAX_KEY_ID_BYTES || key_id.trim() != key_id {
        return Err(RejectReason::Key);
    }
    Ok(())
}

/// Verify the RS256 signature over the exact compact segments, then decode claims.
fn verify_rs256(token: &str, jwk: &PublicJwk) -> Result<OidcClaims, RejectReason> {
    let (header, claims, signature_bytes) = segments(token)?;
    let modulus = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(jwk.modulus.as_str())
        .map_err(|_error| RejectReason::Key)?;
    let exponent = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(jwk.exponent.as_str())
        .map_err(|_error| RejectReason::Key)?;
    let signature = decode_segment(signature_bytes)?;
    let key = signature::RsaPublicKeyComponents {
        n: modulus.as_slice(),
        e: exponent.as_slice(),
    };
    let signed = format!("{header}.{claims}");
    key.verify(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        signed.as_bytes(),
        &signature,
    )
    .map_err(|_error| RejectReason::Signature)?;
    serde_json::from_slice(&decode_segment(claims)?).map_err(|_error| RejectReason::MalformedToken)
}

fn validate_claim_shapes(claims: &OidcClaims) -> Result<(), RejectReason> {
    for value in [
        claims.iss.as_str(),
        claims.sub.as_str(),
        claims.jti.as_str(),
        claims.repository_owner.as_str(),
        claims.repository.as_str(),
        claims.job_workflow_ref.as_str(),
        claims.sha.as_str(),
        claims.git_ref.as_str(),
    ] {
        if !bounded_claim(value) {
            return Err(RejectReason::ClaimType);
        }
    }
    if claims.sub.len() > MAX_SUBJECT_BYTES || !claims.aud.is_bounded() {
        return Err(RejectReason::ClaimType);
    }
    if claims.sha.len() != GIT_COMMIT_HEX_BYTES
        || !claims.sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(RejectReason::ClaimType);
    }
    Ok(())
}

fn validate_identity_claims(
    claims: &OidcClaims,
    now_seconds: u64,
    policy: &OidcPolicy,
) -> Result<(), RejectReason> {
    if claims.iss != policy.issuer_url {
        return Err(RejectReason::Issuer);
    }
    if !policy
        .audiences
        .iter()
        .any(|audience| claims.aud.contains(audience))
    {
        return Err(RejectReason::Audience);
    }
    validate_time_claims(claims, now_seconds, policy)?;
    if claims.repository_owner != policy.repository_owner
        || claims.repository != policy.repository
        || !claims
            .job_workflow_ref
            .starts_with(policy.workflow_ref_prefix.as_str())
        || !claims.git_ref.starts_with(policy.ref_prefix.as_str())
    {
        return Err(RejectReason::ClaimMismatch);
    }
    Ok(())
}

fn validate_time_claims(
    claims: &OidcClaims,
    now_seconds: u64,
    policy: &OidcPolicy,
) -> Result<(), RejectReason> {
    let skew = policy.time.clock_skew_seconds;
    let latest_accepted = now_seconds.checked_add(skew).ok_or(RejectReason::Time)?;
    let oldest_accepted = now_seconds
        .saturating_sub(policy.time.max_token_age_seconds)
        .saturating_sub(skew);
    if claims.iat > latest_accepted
        || claims.nbf > latest_accepted
        || claims.exp <= now_seconds.saturating_sub(skew)
        || claims.iat < oldest_accepted
        || claims.nbf > claims.exp
        || claims.iat > claims.exp
    {
        return Err(RejectReason::Time);
    }
    Ok(())
}

fn attenuate(
    claims: &OidcClaims,
    jwk: &PublicJwk,
    now_seconds: u64,
    policy: &OidcPolicy,
    request: &ExchangeRequest,
) -> Result<IssuancePlan, RejectReason> {
    if request.audience != policy.capability.audience {
        return Err(RejectReason::CapabilityAudience);
    }
    if request.project != policy.capability.project {
        return Err(RejectReason::CapabilityProject);
    }
    if !policy.capability.operations.contains(&request.operation) {
        return Err(RejectReason::CapabilityOperation);
    }
    if !resource_is_within(
        request.resource.as_str(),
        policy.capability.resource_prefix.as_str(),
    ) {
        return Err(RejectReason::CapabilityResource);
    }
    if request.lifetime_seconds == 0
        || request.lifetime_seconds > policy.time.max_ucan_lifetime_seconds
    {
        return Err(RejectReason::CapabilityLifetime);
    }
    let requested_expiration = now_seconds
        .checked_add(request.lifetime_seconds)
        .ok_or(RejectReason::CapabilityLifetime)?;
    let expires_at_seconds = requested_expiration.min(claims.exp);
    if expires_at_seconds <= now_seconds {
        return Err(RejectReason::CapabilityLifetime);
    }
    if policy.replay.require_jti && claims.jti.is_empty() {
        return Err(RejectReason::ReplayFacts);
    }
    let subject = VerifiedSubject {
        identity: subject_identity(claims),
        subject: claims.sub.clone(),
        repository_owner: claims.repository_owner.clone(),
        repository: claims.repository.clone(),
        workflow_ref: claims.job_workflow_ref.clone(),
        commit: claims.sha.clone(),
        git_ref: claims.git_ref.clone(),
    };
    let replay_key = replay_identity(claims, policy, request);
    Ok(IssuancePlan {
        subject,
        policy_identity: policy.identity.clone(),
        key_id: jwk.key_id.clone(),
        audience: request.audience.clone(),
        project: request.project.clone(),
        operation: request.operation,
        resource: request.resource.clone(),
        not_before_seconds: now_seconds,
        expires_at_seconds,
        replay_key,
    })
}

fn resource_is_within(resource: &str, prefix: &str) -> bool {
    resource.starts_with(prefix)
        && (prefix.ends_with('/')
            || resource.len() == prefix.len()
            || resource.as_bytes().get(prefix.len()) == Some(&b'/'))
}

fn subject_identity(claims: &OidcClaims) -> String {
    framed_identity(
        b"basalt.oidc.subject.v1",
        &[
            (b"issuer", claims.iss.as_str()),
            (b"subject", claims.sub.as_str()),
            (b"repository", claims.repository.as_str()),
            (b"workflow", claims.job_workflow_ref.as_str()),
            (b"commit", claims.sha.as_str()),
        ],
    )
}

fn replay_identity(
    claims: &OidcClaims,
    policy: &OidcPolicy,
    request: &ExchangeRequest,
) -> ReplayKey {
    ReplayKey::new(framed_identity(
        b"basalt.oidc.replay.v1",
        &[
            (b"issuer", claims.iss.as_str()),
            (b"jti", claims.jti.as_str()),
            (b"audience", request.audience.as_str()),
            (b"project", request.project.as_str()),
            (b"operation", request.operation.ability()),
            (b"resource", request.resource.as_str()),
            (b"policy", policy.identity.as_str()),
        ],
    ))
}

fn framed_identity(domain: &[u8], fields: &[(&[u8], &str)]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for (tag, value) in fields {
        update_frame(&mut hasher, tag);
        update_frame(&mut hasher, value.as_bytes());
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

fn update_frame(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(&length.to_le_bytes());
    hasher.update(value);
}

fn bounded_claim(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= super::model::claim_text_limit()
        && value.trim() == value
        && value.bytes().all(|byte| !byte.is_ascii_control())
}
