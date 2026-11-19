use std::collections::BTreeSet;

use base64::Engine as _;
use bounded_http::core::{
    AddressPolicy, BodyCompletion, CanonicalRequest, CanonicalUrl, DeadlineProfile,
    DestinationProfile, HostPattern, HttpMethod, OperationId, PublicHeader, RedirectProfile,
    RequestDraft, RequestProfiles, ResourceLimits, RetryProfile, TlsProfile, TlsVersion,
};
use bounded_http::{
    Client, ClientError, ClientPorts, CodecFailureClass, ExecutionResult, MemoryCredentials,
    RustlsTls, SystemClock, SystemResolver, TcpTransport, ThreadTimer,
};
use serde::Deserialize;

use super::core::inspect_token_key_id;
use super::model::{OidcPolicy, PublicJwk, RejectReason};

const HTTPS_PORT: u16 = 443;
const REQUEST_BODY_BYTES: u64 = 1;
const RESPONSE_HEADER_BYTES: u64 = 64 * 1024;
const RESPONSE_HEADER_COUNT: u32 = 128;
const RESPONSE_CHUNK_BYTES: u64 = 64 * 1024;
const DECOMPRESSION_RATIO: u32 = 16;
const RESOLVER_CANDIDATES: u32 = 8;
const CONNECTIONS: u32 = 4;
const RSA_MINIMUM_MODULUS_BYTES: usize = 256;
const RSA_MAXIMUM_MODULUS_BYTES: usize = 1024;
const RSA_MAXIMUM_EXPONENT_BYTES: usize = 8;
const MAX_KEY_TEXT_BYTES: usize = 16 * 1024;
const CACHE_CONTROL_MAX_AGE: &str = "max-age=";
const JSON_MEDIA_TYPE: &str = "application/json";

/// Discovery or JWKS document kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    /// OIDC discovery metadata.
    Discovery,
    /// JSON Web Key Set.
    Jwks,
}

/// Stable document retrieval failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFailure {
    /// DNS resolution failed or exceeded its bound.
    Dns,
    /// TLS setup or certificate validation failed.
    Tls,
    /// A redirect was denied.
    Redirect,
    /// A stage or total deadline expired.
    Timeout,
    /// The body or response metadata exceeded a limit.
    Oversized,
    /// The endpoint returned an unusable status or media type.
    Unavailable,
    /// The response body was malformed.
    Malformed,
}

/// One bounded HTTP document response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentResponse {
    /// HTTP status.
    pub status: u16,
    /// Normalized media type.
    pub content_type: String,
    /// Complete bounded body.
    pub body: Vec<u8>,
    /// Observed cache freshness, in seconds.
    pub fresh_seconds: u64,
}

/// External document capability used by the JWKS shell.
pub trait DocumentPort {
    /// Fetch one exact configured URL under the supplied body limit.
    ///
    /// # Errors
    ///
    /// Returns a bounded transport or response failure.
    fn fetch(
        &mut self,
        kind: DocumentKind,
        url: &str,
        maximum_body_bytes: u64,
    ) -> Result<DocumentResponse, DocumentFailure>;
}

/// Bounded HTTP production adapter for OIDC discovery and JWKS documents.
pub struct BoundedHttpDocumentPort {
    client: Client,
    profiles: RequestProfiles,
}

impl BoundedHttpDocumentPort {
    /// Build a public-network HTTPS adapter from admitted OIDC policy.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentFailure::Unavailable`] when policy URLs cannot form one bounded profile.
    pub fn new(policy: &OidcPolicy) -> Result<Self, DocumentFailure> {
        let discovery = CanonicalUrl::parse_https(policy.discovery_url.as_str())
            .map_err(|_error| DocumentFailure::Unavailable)?;
        let jwks = CanonicalUrl::parse_https(policy.jwks_url.as_str())
            .map_err(|_error| DocumentFailure::Unavailable)?;
        let mut hosts = vec![
            HostPattern::exact(discovery.host()).map_err(|_error| DocumentFailure::Unavailable)?,
        ];
        if discovery.host() != jwks.host() {
            hosts.push(
                HostPattern::exact(jwks.host()).map_err(|_error| DocumentFailure::Unavailable)?,
            );
        }
        let body_limit = policy
            .network
            .discovery_body_bytes
            .max(policy.network.jwks_body_bytes);
        let profiles = RequestProfiles {
            destination: DestinationProfile::new(
                policy.network.transport_profile_id.as_str(),
                hosts,
                vec![HTTPS_PORT],
                AddressPolicy::public_only(),
            )
            .map_err(|_error| DocumentFailure::Unavailable)?,
            tls: TlsProfile::new("basalt.oidc.tls13.v1", TlsVersion::Tls13)
                .map_err(|_error| DocumentFailure::Unavailable)?,
            redirect: RedirectProfile::deny(),
            deadlines: DeadlineProfile {
                resolve_ms: policy.network.dns_ms,
                connect_ms: policy.network.connect_ms,
                tls_ms: policy.network.tls_ms,
                first_byte_ms: policy.network.first_byte_ms,
                idle_ms: policy.network.body_ms,
                body_ms: policy.network.body_ms,
                total_ms: policy.network.total_ms,
            },
            retry: RetryProfile {
                max_attempts: 1,
                delay_ms: 0,
            },
            resources: ResourceLimits {
                request_body_bytes: REQUEST_BODY_BYTES,
                wire_response_bytes: body_limit,
                decoded_response_bytes: body_limit,
                retained_prefix_bytes: body_limit,
                header_bytes: RESPONSE_HEADER_BYTES,
                header_count: RESPONSE_HEADER_COUNT,
                chunk_bytes: RESPONSE_CHUNK_BYTES.min(body_limit),
                decompression_ratio: DECOMPRESSION_RATIO,
                resolver_candidates: RESOLVER_CANDIDATES,
                connections: CONNECTIONS,
            },
        };
        profiles
            .resources
            .validate()
            .map_err(|_error| DocumentFailure::Unavailable)?;
        profiles
            .deadlines
            .validate()
            .map_err(|_error| DocumentFailure::Unavailable)?;
        Ok(Self {
            client: Client::new(ClientPorts {
                resolver: Box::new(SystemResolver::default()),
                transport: Box::new(TcpTransport),
                tls: Box::new(RustlsTls::default()),
                clock: Box::new(SystemClock::new()),
                timer: Box::new(ThreadTimer),
                credentials: Box::new(MemoryCredentials::new(Vec::new())),
            }),
            profiles,
        })
    }
}

impl DocumentPort for BoundedHttpDocumentPort {
    fn fetch(
        &mut self,
        kind: DocumentKind,
        url: &str,
        maximum_body_bytes: u64,
    ) -> Result<DocumentResponse, DocumentFailure> {
        let mut profiles = self.profiles.clone();
        profiles.resources.wire_response_bytes = maximum_body_bytes;
        profiles.resources.decoded_response_bytes = maximum_body_bytes;
        profiles.resources.retained_prefix_bytes = maximum_body_bytes;
        profiles.resources.chunk_bytes = profiles.resources.chunk_bytes.min(maximum_body_bytes);
        let request = CanonicalRequest::admit(RequestDraft {
            method: HttpMethod::Get,
            url: CanonicalUrl::parse_https(url).map_err(|_error| DocumentFailure::Unavailable)?,
            public_headers: vec![
                PublicHeader::new("accept", JSON_MEDIA_TYPE)
                    .map_err(|_error| DocumentFailure::Unavailable)?,
            ],
            secret_headers: Vec::new(),
            body: Vec::new(),
            operation: OperationId::new(match kind {
                DocumentKind::Discovery => "basalt.oidc.discovery",
                DocumentKind::Jwks => "basalt.oidc.jwks",
            })
            .map_err(|_error| DocumentFailure::Unavailable)?,
            idempotency: None,
            profiles,
        })
        .map_err(|_error| DocumentFailure::Unavailable)?;
        match self.client.execute(&request) {
            ExecutionResult::Response { observation, .. } => {
                if observation.completion() != BodyCompletion::Complete {
                    return Err(DocumentFailure::Oversized);
                }
                let content_type = selected_header(&observation, "content-type")
                    .unwrap_or_default()
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase();
                let fresh_seconds = selected_header(&observation, "cache-control")
                    .and_then(parse_max_age)
                    .unwrap_or(0);
                Ok(DocumentResponse {
                    status: observation.status(),
                    content_type,
                    body: observation.retained_prefix().to_vec(),
                    fresh_seconds,
                })
            }
            ExecutionResult::Failure { error, .. } => Err(map_client_error(error)),
        }
    }
}

fn selected_header<'a>(
    observation: &'a bounded_http::core::ResponseObservation,
    name: &str,
) -> Option<&'a str> {
    observation
        .selected_headers()
        .iter()
        .find(|header| header.name().as_str() == name)
        .map(PublicHeader::value)
}

fn parse_max_age(value: &str) -> Option<u64> {
    value.split(',').find_map(|directive| {
        directive
            .trim()
            .strip_prefix(CACHE_CONTROL_MAX_AGE)
            .and_then(|seconds| seconds.parse::<u64>().ok())
    })
}

const fn map_client_error(error: ClientError) -> DocumentFailure {
    match error {
        ClientError::Resolve(bounded_http::ResolverFailure::Timeout)
        | ClientError::Transport(bounded_http::TransportFailure::Timeout)
        | ClientError::Tls(bounded_http::TlsFailure::Timeout)
        | ClientError::Codec(CodecFailureClass::Timeout) => DocumentFailure::Timeout,
        ClientError::Resolve(_) => DocumentFailure::Dns,
        ClientError::Tls(_) => DocumentFailure::Tls,
        ClientError::Redirect(_) | ClientError::RedirectRequest(_) => DocumentFailure::Redirect,
        ClientError::Codec(
            CodecFailureClass::HeaderLimit
            | CodecFailureClass::WireLimit
            | CodecFailureClass::DecodedLimit,
        ) => DocumentFailure::Oversized,
        _ => DocumentFailure::Unavailable,
    }
}

/// Cached key set with explicit freshness.
#[derive(Debug, Default)]
pub struct JwksCache {
    current: Option<CachedJwks>,
}

#[derive(Debug)]
struct CachedJwks {
    keys: Vec<PublicJwk>,
    fresh_until_seconds: u64,
}

/// Stable key-loading failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyLoadFailure {
    /// Token header cannot select a key.
    Token(RejectReason),
    /// Discovery retrieval failed.
    Discovery(DocumentFailure),
    /// Discovery issuer or JWKS identity did not match policy.
    DiscoveryIdentity,
    /// JWKS retrieval failed.
    Jwks(DocumentFailure),
    /// JWKS JSON or key material is malformed.
    MalformedJwks,
    /// Key count exceeded policy.
    ExcessiveKeys,
    /// The selected key is absent or ambiguous after one refresh.
    UnknownKey,
    /// Existing keys are stale and refresh failed.
    Stale,
}

/// Load one exact public key, with at most one refresh for an unknown key.
///
/// # Errors
///
/// Returns a typed failure for token, transport, identity, freshness, parse, count, or key errors.
pub fn load_verification_key(
    token: &str,
    now_seconds: u64,
    policy: &OidcPolicy,
    cache: &mut JwksCache,
    documents: &mut dyn DocumentPort,
) -> Result<PublicJwk, KeyLoadFailure> {
    let key_id =
        inspect_token_key_id(token, policy.network.token_bytes).map_err(KeyLoadFailure::Token)?;
    if let Some(cached) = &cache.current
        && now_seconds <= cached.fresh_until_seconds
        && let Some(key) = unique_key(cached.keys.as_slice(), key_id.as_str())?
    {
        return Ok(key.clone());
    }
    let refreshed = refresh_keys(now_seconds, policy, documents);
    let keys = match refreshed {
        Ok(value) => value,
        Err(error) => {
            if cache.current.is_some() {
                return Err(if policy.cache.allow_stale_on_error {
                    error
                } else {
                    KeyLoadFailure::Stale
                });
            }
            return Err(error);
        }
    };
    let selected = unique_key(keys.keys.as_slice(), key_id.as_str())?
        .cloned()
        .ok_or(KeyLoadFailure::UnknownKey)?;
    cache.current = Some(keys);
    Ok(selected)
}

fn refresh_keys(
    now_seconds: u64,
    policy: &OidcPolicy,
    documents: &mut dyn DocumentPort,
) -> Result<CachedJwks, KeyLoadFailure> {
    let discovery = documents
        .fetch(
            DocumentKind::Discovery,
            policy.discovery_url.as_str(),
            policy.network.discovery_body_bytes,
        )
        .map_err(KeyLoadFailure::Discovery)?;
    validate_document_response(&discovery).map_err(KeyLoadFailure::Discovery)?;
    let metadata: DiscoveryDocument = serde_json::from_slice(discovery.body.as_slice())
        .map_err(|_error| KeyLoadFailure::Discovery(DocumentFailure::Malformed))?;
    if metadata.issuer != policy.issuer_url || metadata.jwks_uri != policy.jwks_url {
        return Err(KeyLoadFailure::DiscoveryIdentity);
    }
    let response = documents
        .fetch(
            DocumentKind::Jwks,
            policy.jwks_url.as_str(),
            policy.network.jwks_body_bytes,
        )
        .map_err(KeyLoadFailure::Jwks)?;
    validate_document_response(&response).map_err(KeyLoadFailure::Jwks)?;
    let document: JwksDocument = serde_json::from_slice(response.body.as_slice())
        .map_err(|_error| KeyLoadFailure::MalformedJwks)?;
    let maximum_keys = usize::try_from(policy.network.jwks_keys)
        .map_err(|_error| KeyLoadFailure::ExcessiveKeys)?;
    if document.keys.is_empty() || document.keys.len() > maximum_keys {
        return Err(KeyLoadFailure::ExcessiveKeys);
    }
    let keys = admit_keys(document.keys)?;
    let freshness = response.fresh_seconds.min(policy.cache.max_fresh_seconds);
    let fresh_until_seconds = now_seconds.saturating_add(freshness);
    Ok(CachedJwks {
        keys,
        fresh_until_seconds,
    })
}

fn validate_document_response(response: &DocumentResponse) -> Result<(), DocumentFailure> {
    if response.status != 200
        || response.content_type != JSON_MEDIA_TYPE
        || response.body.is_empty()
    {
        return Err(DocumentFailure::Unavailable);
    }
    Ok(())
}

fn unique_key<'a>(
    keys: &'a [PublicJwk],
    key_id: &str,
) -> Result<Option<&'a PublicJwk>, KeyLoadFailure> {
    let mut matches = keys.iter().filter(|key| key.key_id == key_id);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(KeyLoadFailure::UnknownKey);
    }
    Ok(first)
}

fn admit_keys(raw: Vec<RawJwk>) -> Result<Vec<PublicJwk>, KeyLoadFailure> {
    let mut seen = BTreeSet::new();
    let mut admitted = Vec::with_capacity(raw.len());
    for key in raw {
        if key.key_type != "RSA"
            || key.algorithm.as_deref() != Some("RS256")
            || key.public_use.as_deref() != Some("sig")
            || key.key_id.is_empty()
            || key.key_id.len() > MAX_KEY_TEXT_BYTES
            || key.modulus.is_empty()
            || key.modulus.len() > MAX_KEY_TEXT_BYTES
            || key.exponent.is_empty()
            || key.exponent.len() > MAX_KEY_TEXT_BYTES
            || key
                .key_operations
                .as_ref()
                .is_some_and(|operations| operations.as_slice() != ["verify"])
            || !seen.insert(key.key_id.clone())
        {
            return Err(KeyLoadFailure::MalformedJwks);
        }
        let modulus = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(key.modulus.as_bytes())
            .map_err(|_error| KeyLoadFailure::MalformedJwks)?;
        let exponent = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(key.exponent.as_bytes())
            .map_err(|_error| KeyLoadFailure::MalformedJwks)?;
        if !(RSA_MINIMUM_MODULUS_BYTES..=RSA_MAXIMUM_MODULUS_BYTES).contains(&modulus.len())
            || exponent.is_empty()
            || exponent.len() > RSA_MAXIMUM_EXPONENT_BYTES
        {
            return Err(KeyLoadFailure::MalformedJwks);
        }
        admitted.push(PublicJwk {
            key_id: key.key_id,
            modulus: key.modulus,
            exponent: key.exponent,
        });
    }
    Ok(admitted)
}

#[derive(Debug, Deserialize)]
struct DiscoveryDocument {
    issuer: String,
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct JwksDocument {
    keys: Vec<RawJwk>,
}

#[derive(Debug, Deserialize)]
struct RawJwk {
    #[serde(rename = "kty")]
    key_type: String,
    #[serde(rename = "use")]
    public_use: Option<String>,
    #[serde(rename = "alg")]
    algorithm: Option<String>,
    #[serde(rename = "kid")]
    key_id: String,
    #[serde(rename = "key_ops")]
    key_operations: Option<Vec<String>>,
    #[serde(rename = "n")]
    modulus: String,
    #[serde(rename = "e")]
    exponent: String,
}
