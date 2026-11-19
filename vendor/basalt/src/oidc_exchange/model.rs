use serde::{Deserialize, Serialize};

const OIDC_POLICY_SCHEMA: &str = "basalt.oidc-exchange.policy.v1";
const REQUIRED_ALGORITHM: &str = "RS256";
const REQUIRED_KEY_TYPE: &str = "RSA";
const REQUIRED_KEY_USE: &str = "sig";
const MAX_POLICY_ITEMS: usize = 64;
const MAX_TEXT_BYTES: usize = 4096;

pub(super) const fn policy_item_limit() -> usize {
    MAX_POLICY_ITEMS
}

pub(super) const fn claim_text_limit() -> usize {
    MAX_TEXT_BYTES
}
const MAX_TOKEN_BYTES: u64 = 32 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;
const MAX_KEYS: u32 = 128;
const MAX_SECONDS: u64 = 24 * 60 * 60;

/// Stable policy admission failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    /// JSON shape or type mismatch.
    Json,
    /// The schema identifier is unsupported.
    Schema,
    /// One required text value is empty or excessive.
    Text,
    /// A count or byte bound is zero or excessive.
    Bound,
    /// An HTTPS origin or URL is invalid.
    Url,
    /// The policy enables an unsupported algorithm.
    Algorithm,
    /// Capability policy is empty or contradictory.
    Capability,
    /// Required claim or non-claim policy is absent.
    ClaimBoundary,
}

/// OIDC exchange policy admitted from typed Nickel output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcPolicy {
    /// BLAKE3 identity of the exact exported policy bytes.
    pub identity: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Trusted issuer identifier.
    pub issuer_id: String,
    /// Exact HTTPS issuer URL.
    pub issuer_url: String,
    /// Exact HTTPS discovery URL.
    pub discovery_url: String,
    /// Exact HTTPS JWKS URL expected from discovery.
    pub jwks_url: String,
    /// Accepted OIDC audiences.
    pub audiences: Vec<String>,
    /// Required GitHub repository owner.
    pub repository_owner: String,
    /// Required GitHub repository.
    pub repository: String,
    /// Required prefix for `job_workflow_ref`.
    pub workflow_ref_prefix: String,
    /// Required prefix for the Git reference.
    pub ref_prefix: String,
    /// Explicit time bounds.
    pub time: TimePolicy,
    /// Explicit network and document bounds.
    pub network: NetworkPolicy,
    /// Explicit cache policy.
    pub cache: CachePolicy,
    /// Explicit Mantle capability vocabulary.
    pub capability: CapabilityPolicy,
    /// Explicit replay policy.
    pub replay: ReplayPolicy,
}

/// Explicit time limits in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimePolicy {
    /// Accepted clock skew.
    pub clock_skew_seconds: u64,
    /// Maximum age since `iat`.
    pub max_token_age_seconds: u64,
    /// Maximum issued UCAN lifetime.
    pub max_ucan_lifetime_seconds: u64,
}

/// Explicit HTTPS and document limits.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkPolicy {
    /// Bounded HTTP profile identity.
    pub transport_profile_id: String,
    /// Maximum compact OIDC token bytes.
    pub token_bytes: u64,
    /// Maximum discovery body bytes.
    pub discovery_body_bytes: u64,
    /// Maximum JWKS body bytes.
    pub jwks_body_bytes: u64,
    /// Maximum JWK count.
    pub jwks_keys: u32,
    /// Maximum redirects. The initial profile requires zero.
    pub redirects: u32,
    /// Maximum refreshes after an unknown key.
    pub refreshes: u32,
    /// DNS deadline.
    pub dns_ms: u64,
    /// Connection deadline.
    pub connect_ms: u64,
    /// TLS deadline.
    pub tls_ms: u64,
    /// First-byte deadline.
    pub first_byte_ms: u64,
    /// Body deadline.
    pub body_ms: u64,
    /// Total request deadline.
    pub total_ms: u64,
}

/// Bounded JWKS cache policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CachePolicy {
    /// Maximum accepted freshness from a response.
    pub max_fresh_seconds: u64,
    /// Whether stale keys can be used during an outage. The initial profile denies this.
    pub allow_stale_on_error: bool,
}

/// Versioned Mantle capability vocabulary and scope.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPolicy {
    /// Vocabulary schema.
    pub vocabulary: String,
    /// Exact Mantle UCAN audience DID.
    pub audience: String,
    /// Exact project identifier.
    pub project: String,
    /// Admitted operations.
    pub operations: Vec<MantleOperation>,
    /// Required resource prefix.
    pub resource_prefix: String,
}

/// Replay policy for an exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayPolicy {
    /// Require a one-time exchange.
    pub require_one_time: bool,
    /// Require an issuer-provided `jti` rather than a token digest.
    pub require_jti: bool,
}

/// Closed Mantle operation vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MantleOperation {
    /// Submit a build.
    BuildSubmit,
    /// Upload one admitted build input.
    InputUpload,
    /// Read one store object.
    StoreRead,
    /// Read build status.
    StatusRead,
    /// Read build logs.
    LogRead,
    /// Cancel an attempt owned by the mapped project.
    OwnedAttemptCancel,
}

impl MantleOperation {
    /// Return the stable UCAN ability.
    #[must_use]
    pub const fn ability(self) -> &'static str {
        match self {
            Self::BuildSubmit => "mantle/build/submit",
            Self::InputUpload => "mantle/input/upload",
            Self::StoreRead => "mantle/store/read",
            Self::StatusRead => "mantle/status/read",
            Self::LogRead => "mantle/log/read",
            Self::OwnedAttemptCancel => "mantle/attempt/cancel-owned",
        }
    }
}

/// One requested capability exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeRequest {
    /// Exact target audience.
    pub audience: String,
    /// Exact project.
    pub project: String,
    /// Closed operation.
    pub operation: MantleOperation,
    /// Exact requested resource.
    pub resource: String,
    /// Requested UCAN lifetime.
    pub lifetime_seconds: u64,
}

/// One admitted public RSA JWK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicJwk {
    /// Public key identifier.
    pub key_id: String,
    /// Base64url RSA modulus.
    pub modulus: String,
    /// Base64url RSA exponent.
    pub exponent: String,
}

/// Public subject facts accepted from OIDC claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedSubject {
    /// Stable BLAKE3 subject identity.
    pub identity: String,
    /// GitHub subject claim.
    pub subject: String,
    /// Repository owner.
    pub repository_owner: String,
    /// Repository.
    pub repository: String,
    /// Workflow reference.
    pub workflow_ref: String,
    /// Git commit.
    pub commit: String,
    /// Git reference.
    pub git_ref: String,
}

/// Pure plan accepted for UCAN issuance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuancePlan {
    /// Verified public subject.
    pub subject: VerifiedSubject,
    /// Policy identity.
    pub policy_identity: String,
    /// OIDC key identifier.
    pub key_id: String,
    /// Exact UCAN audience.
    pub audience: String,
    /// Exact granted project.
    pub project: String,
    /// Exact granted operation.
    pub operation: MantleOperation,
    /// Exact granted resource.
    pub resource: String,
    /// Issuance not-before time.
    pub not_before_seconds: u64,
    /// Issuance expiration time.
    pub expires_at_seconds: u64,
    /// Replay key derived from public facts only.
    pub replay_key: ReplayKey,
}

/// BLAKE3 replay identity over public token and request facts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplayKey(String);

impl ReplayKey {
    pub(super) const fn new(value: String) -> Self {
        Self(value)
    }

    /// Return the stable replay identity.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable verification or attenuation rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RejectReason {
    /// Compact token shape or JSON is malformed.
    MalformedToken,
    /// Token exceeds the configured byte limit.
    TokenTooLarge,
    /// Token algorithm is unsupported or mismatched.
    Algorithm,
    /// Token type is absent or unsupported.
    TokenType,
    /// Token key identifier is absent or mismatched.
    Key,
    /// JWK use, operations, type, or algorithm are invalid.
    KeyUse,
    /// Signature validation failed.
    Signature,
    /// Issuer does not match policy.
    Issuer,
    /// Audience does not match policy.
    Audience,
    /// Required time claims are absent or invalid.
    Time,
    /// A required claim has the wrong shape.
    ClaimType,
    /// Selected identity claims do not match policy.
    ClaimMismatch,
    /// Requested Mantle audience is outside policy.
    CapabilityAudience,
    /// Requested project is outside policy.
    CapabilityProject,
    /// Requested operation is outside policy.
    CapabilityOperation,
    /// Requested resource is outside policy.
    CapabilityResource,
    /// Requested lifetime exceeds policy or OIDC validity.
    CapabilityLifetime,
    /// Required replay facts are absent.
    ReplayFacts,
}

impl RejectReason {
    /// Return the stable reason string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MalformedToken => "malformed-token",
            Self::TokenTooLarge => "token-too-large",
            Self::Algorithm => "algorithm",
            Self::TokenType => "token-type",
            Self::Key => "key",
            Self::KeyUse => "key-use",
            Self::Signature => "signature",
            Self::Issuer => "issuer",
            Self::Audience => "audience",
            Self::Time => "time",
            Self::ClaimType => "claim-type",
            Self::ClaimMismatch => "claim-mismatch",
            Self::CapabilityAudience => "capability-audience",
            Self::CapabilityProject => "capability-project",
            Self::CapabilityOperation => "capability-operation",
            Self::CapabilityResource => "capability-resource",
            Self::CapabilityLifetime => "capability-lifetime",
            Self::ReplayFacts => "replay-facts",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDocument {
    schema: String,
    profile_id: String,
    issuer: IssuerPolicyDocument,
    audiences: Vec<String>,
    algorithm: String,
    claims: ClaimsPolicyDocument,
    time: TimePolicy,
    network: NetworkPolicy,
    cache: CachePolicy,
    capability: CapabilityPolicy,
    replay: ReplayPolicy,
    non_claims: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct IssuerPolicyDocument {
    id: String,
    issuer_url: String,
    discovery_url: String,
    jwks_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimsPolicyDocument {
    repository_owner: String,
    repository: String,
    workflow_ref_prefix: String,
    ref_prefix: String,
}

const REQUIRED_NON_CLAIMS: [&str; 7] = [
    "does not acquire OIDC tokens",
    "does not grant authority from signature validity alone",
    "does not own DNS TLS or remote truth",
    "does not own signing key custody",
    "does not own replay storage",
    "does not prove provider or deployment trust",
    "does not grant Mantle execution or release authority",
];

/// Parse and admit one Nickel-exported OIDC policy.
///
/// # Errors
///
/// Returns a stable error for malformed, unsupported, empty, excessive, or contradictory policy.
pub fn parse_policy(input: &str) -> Result<OidcPolicy, PolicyError> {
    let document: PolicyDocument =
        serde_json::from_str(input).map_err(|_error| PolicyError::Json)?;
    validate_document(&document)?;
    Ok(OidcPolicy {
        identity: format!("blake3:{}", blake3::hash(input.as_bytes()).to_hex()),
        profile_id: document.profile_id,
        issuer_id: document.issuer.id,
        issuer_url: document.issuer.issuer_url,
        discovery_url: document.issuer.discovery_url,
        jwks_url: document.issuer.jwks_url,
        audiences: document.audiences,
        repository_owner: document.claims.repository_owner,
        repository: document.claims.repository,
        workflow_ref_prefix: document.claims.workflow_ref_prefix,
        ref_prefix: document.claims.ref_prefix,
        time: document.time,
        network: document.network,
        cache: document.cache,
        capability: document.capability,
        replay: document.replay,
    })
}

fn validate_document(document: &PolicyDocument) -> Result<(), PolicyError> {
    if document.schema != OIDC_POLICY_SCHEMA {
        return Err(PolicyError::Schema);
    }
    for text in [
        document.profile_id.as_str(),
        document.issuer.id.as_str(),
        document.issuer.issuer_url.as_str(),
        document.issuer.discovery_url.as_str(),
        document.issuer.jwks_url.as_str(),
        document.claims.repository_owner.as_str(),
        document.claims.repository.as_str(),
        document.claims.workflow_ref_prefix.as_str(),
        document.claims.ref_prefix.as_str(),
        document.network.transport_profile_id.as_str(),
        document.capability.vocabulary.as_str(),
        document.capability.audience.as_str(),
        document.capability.project.as_str(),
        document.capability.resource_prefix.as_str(),
    ] {
        if !bounded_text(text) {
            return Err(PolicyError::Text);
        }
    }
    if !exact_https_url(&document.issuer.issuer_url)
        || !exact_https_url(&document.issuer.discovery_url)
        || !exact_https_url(&document.issuer.jwks_url)
    {
        return Err(PolicyError::Url);
    }
    if document.algorithm != REQUIRED_ALGORITHM {
        return Err(PolicyError::Algorithm);
    }
    if document.audiences.is_empty()
        || document.audiences.len() > MAX_POLICY_ITEMS
        || document.audiences.iter().any(|value| !bounded_text(value))
    {
        return Err(PolicyError::Bound);
    }
    validate_time(document.time)?;
    validate_network(&document.network)?;
    if document.cache.max_fresh_seconds == 0
        || document.cache.max_fresh_seconds > MAX_SECONDS
        || document.cache.allow_stale_on_error
    {
        return Err(PolicyError::Bound);
    }
    if document.capability.vocabulary != "onix.mantle.capability.v1"
        || document.capability.operations.is_empty()
        || document.capability.operations.len() > MAX_POLICY_ITEMS
    {
        return Err(PolicyError::Capability);
    }
    let mut operations = document.capability.operations.clone();
    operations.sort_unstable();
    operations.dedup();
    if operations.len() != document.capability.operations.len() {
        return Err(PolicyError::Capability);
    }
    if !document.replay.require_one_time || !document.replay.require_jti {
        return Err(PolicyError::ClaimBoundary);
    }
    if document.non_claims.as_slice() != REQUIRED_NON_CLAIMS {
        return Err(PolicyError::ClaimBoundary);
    }
    let _ = (REQUIRED_KEY_TYPE, REQUIRED_KEY_USE);
    Ok(())
}

const fn validate_time(time: TimePolicy) -> Result<(), PolicyError> {
    if time.max_token_age_seconds == 0
        || time.max_ucan_lifetime_seconds == 0
        || time.max_token_age_seconds > MAX_SECONDS
        || time.max_ucan_lifetime_seconds > MAX_SECONDS
        || time.clock_skew_seconds > time.max_token_age_seconds
    {
        return Err(PolicyError::Bound);
    }
    Ok(())
}

fn validate_network(network: &NetworkPolicy) -> Result<(), PolicyError> {
    let byte_bounds = [
        network.token_bytes,
        network.discovery_body_bytes,
        network.jwks_body_bytes,
    ];
    let time_bounds = [
        network.dns_ms,
        network.connect_ms,
        network.tls_ms,
        network.first_byte_ms,
        network.body_ms,
        network.total_ms,
    ];
    if byte_bounds.contains(&0)
        || byte_bounds.iter().any(|bound| *bound > MAX_DOCUMENT_BYTES)
        || network.token_bytes > MAX_TOKEN_BYTES
        || time_bounds.contains(&0)
        || network.jwks_keys == 0
        || network.jwks_keys > MAX_KEYS
        || network.redirects != 0
        || network.refreshes != 1
        || time_bounds.iter().any(|bound| *bound > network.total_ms)
    {
        return Err(PolicyError::Bound);
    }
    Ok(())
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES && value.trim() == value
}

fn exact_https_url(value: &str) -> bool {
    value.starts_with("https://")
        && !value.contains('@')
        && !value.contains('#')
        && value.bytes().all(|byte| !byte.is_ascii_control())
}
