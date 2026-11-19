use std::fmt;

use crate::model::AdapterBlake3;
use crate::model::BoundaryClaims;
use crate::model::NostrEvent;
use crate::model::NostrEventId;
use crate::model::NostrPublicKey;
use crate::model::VerificationIssue;
use crate::model::validate_boundary;
use crate::protocol::VerifiedNostrEvent;
use crate::protocol::verify_event;

/// NIP-42 client-authentication event kind.
pub const NIP42_EVENT_KIND: u32 = 22_242;
/// NIP-98 HTTP-authentication event kind.
pub const NIP98_EVENT_KIND: u32 = 27_235;
/// Stable NIP-42 purpose identity.
pub const NIP42_PURPOSE: &str = "nostr-relay-auth";
/// Stable NIP-98 purpose identity.
pub const NIP98_PURPOSE: &str = "nostr-http-auth";
/// Stable application profile identity.
pub const APPLICATION_PURPOSE_PROFILE: &str = "ucan-nostr.application-v1";
/// Largest admitted context field.
pub const MAX_CONTEXT_FIELD_BYTES: usize = 4_096;

const REQUEST_REFERENCE_DOMAIN: &str = "ucan-nostr.request-reference.v1";
const PROFILE_IDENTITY_DOMAIN: &str = "ucan-nostr.profile-identity.v1";
const IMPLEMENTATION_IDENTITY_DOMAIN: &str = "ucan-nostr.implementation-identity.v1";
const IMPLEMENTATION_REVISION: &str = "ucan-nostr@0.1.0|k256@0.13.4|sha2@0.10.9|blake3@1.8.5|serde_json@1.0.149";

const NON_CLAIMS: &[&str] = &[
    "verified Nostr key control does not identify a person or product subject",
    "verified Nostr key control does not grant a UCAN capability or authorization",
    "relay acceptance, relay presence, NIP-05, profile metadata, and npub text are not identity proof",
    "relay access, DNS, signing, clocks, replay storage, subject resolution, and authorization remain caller-owned",
];

const _: () = assert!(MAX_CONTEXT_FIELD_BYTES > 0);

/// Caller-supplied event freshness policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessExpectation {
    /// Explicit observed current Unix time in seconds.
    pub observed_now: u64,
    /// Largest accepted event age in seconds.
    pub max_age_seconds: u64,
    /// Largest accepted future skew in seconds.
    pub max_future_skew_seconds: u64,
}

/// Caller-supplied replay-store observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayObservation {
    /// Caller admitted this exact event identity as unused for the request.
    Fresh {
        /// Event identity checked by the caller-owned replay store.
        event_id: NostrEventId,
    },
    /// Caller observed a prior use of this exact event identity.
    Replayed {
        /// Event identity checked by the caller-owned replay store.
        event_id: NostrEventId,
    },
    /// Caller could not obtain a fail-closed replay observation.
    Unavailable,
}

/// Successful freshness disposition retained in verified output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessDisposition {
    /// Event satisfied the exact caller policy.
    Fresh,
}

/// Successful replay disposition retained in verified output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayDisposition {
    /// Caller reported the event unused for this request.
    Fresh,
}

/// Closed authentication profile selected by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationProfile {
    /// NIP-42 relay challenge.
    Nip42Relay,
    /// NIP-98 HTTP request.
    Nip98Http,
    /// Domain-separated application challenge.
    ApplicationV1,
}

/// Exact signed authentication context expected by the caller.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum AuthenticationExpectation<'a> {
    /// NIP-42 relay challenge context.
    Nip42Relay {
        /// Exact relay URL from the signed `relay` tag.
        relay_url: &'a str,
        /// Exact relay challenge from the signed `challenge` tag.
        challenge: &'a str,
        /// Caller-expected audience. It must equal `relay_url`.
        expected_audience: &'a str,
        /// Caller-expected purpose. It must equal [`NIP42_PURPOSE`].
        expected_purpose: &'a str,
    },
    /// NIP-98 HTTP request context.
    Nip98Http {
        /// Exact absolute URL from the signed `u` tag.
        absolute_url: &'a str,
        /// Exact method from the signed `method` tag.
        method: &'a str,
        /// Optional exact SHA-256 payload identity from the signed `payload` tag.
        payload_sha256: Option<NostrEventId>,
        /// Caller-expected audience. It must equal `absolute_url`.
        expected_audience: &'a str,
        /// Caller-expected purpose. It must equal [`NIP98_PURPOSE`].
        expected_purpose: &'a str,
    },
    /// Stack-owned domain-separated application context.
    ApplicationV1 {
        /// Exact application domain from the signed `domain` tag.
        domain: &'a str,
        /// Exact signed audience.
        audience: &'a str,
        /// Exact signed purpose.
        purpose: &'a str,
        /// Exact signed challenge.
        challenge: &'a str,
        /// Exact signed request identity.
        request_identity: &'a str,
        /// Caller-owned application event kind.
        event_kind: u32,
    },
}

impl fmt::Debug for AuthenticationExpectation<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let profile = match self {
            Self::Nip42Relay { .. } => AuthenticationProfile::Nip42Relay,
            Self::Nip98Http { .. } => AuthenticationProfile::Nip98Http,
            Self::ApplicationV1 { .. } => AuthenticationProfile::ApplicationV1,
        };
        formatter
            .debug_struct("AuthenticationExpectation")
            .field("profile", &profile)
            .field("bound_context", &"<redacted>")
            .finish()
    }
}

/// Complete explicit verification request.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerificationRequest<'a> {
    /// Selected authentication context.
    pub authentication: AuthenticationExpectation<'a>,
    /// Explicit freshness inputs.
    pub freshness: FreshnessExpectation,
    /// Explicit replay observation.
    pub replay: ReplayObservation,
    /// Boundary facts that must not promote metadata or secrets.
    pub boundary: BoundaryClaims,
}

impl fmt::Debug for VerificationRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerificationRequest")
            .field("authentication", &self.authentication)
            .field("freshness", &self.freshness)
            .field("replay", &self.replay)
            .field("boundary", &self.boundary)
            .finish()
    }
}

/// Opaque successful proof of Nostr key control for one bound request.
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedNostrCredential {
    public_key: NostrPublicKey,
    event_id: NostrEventId,
    profile: AuthenticationProfile,
    audience: String,
    purpose: String,
    request_reference: AdapterBlake3,
    freshness: FreshnessDisposition,
    replay: ReplayDisposition,
    adapter_profile_identity: AdapterBlake3,
    implementation_identity: AdapterBlake3,
}

impl fmt::Debug for VerifiedNostrCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedNostrCredential")
            .field("public_key", &self.public_key)
            .field("event_id", &self.event_id)
            .field("profile", &self.profile)
            .field("audience", &"<redacted-bound-identity>")
            .field("purpose", &"<redacted-bound-identity>")
            .field("request_reference", &self.request_reference)
            .field("freshness", &self.freshness)
            .field("replay", &self.replay)
            .field("adapter_profile_identity", &self.adapter_profile_identity)
            .field("implementation_identity", &self.implementation_identity)
            .finish()
    }
}

impl VerifiedNostrCredential {
    /// Return the canonical raw Nostr public key.
    #[must_use]
    pub const fn public_key(&self) -> NostrPublicKey {
        self.public_key
    }

    /// Return the NIP-01 SHA-256 event identity.
    #[must_use]
    pub const fn event_id(&self) -> NostrEventId {
        self.event_id
    }

    /// Return the checked authentication profile.
    #[must_use]
    pub const fn profile(&self) -> AuthenticationProfile {
        self.profile
    }

    /// Return the exact bound audience identity.
    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }

    /// Return the exact bound purpose identity.
    #[must_use]
    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    /// Return the adapter-owned BLAKE3 request reference.
    #[must_use]
    pub const fn request_reference(&self) -> AdapterBlake3 {
        self.request_reference
    }

    /// Return the successful freshness disposition.
    #[must_use]
    pub const fn freshness(&self) -> FreshnessDisposition {
        self.freshness
    }

    /// Return the successful replay disposition.
    #[must_use]
    pub const fn replay(&self) -> ReplayDisposition {
        self.replay
    }

    /// Return the selected profile BLAKE3 identity.
    #[must_use]
    pub const fn adapter_profile_identity(&self) -> AdapterBlake3 {
        self.adapter_profile_identity
    }

    /// Return the implementation BLAKE3 identity.
    #[must_use]
    pub const fn implementation_identity(&self) -> AdapterBlake3 {
        self.implementation_identity
    }

    /// Return exact non-claims attached to every credential.
    #[must_use]
    pub const fn non_claims(&self) -> &'static [&'static str] {
        NON_CLAIMS
    }
}

/// Verify protocol identity, signed context, freshness, replay, and boundaries.
///
/// # Errors
///
/// Returns a deterministic issue and never constructs a credential unless all
/// checks pass.
// r[impl ucan.nostr_identity_adapter.authentication_context]
// r[impl ucan.nostr_identity_adapter.credential_boundary]
pub fn verify_credential(
    event: &NostrEvent,
    request: VerificationRequest<'_>,
) -> Result<VerifiedNostrCredential, VerificationIssue> {
    validate_boundary(request.boundary)?;
    validate_context_field_limits(request.authentication)?;
    let verified = verify_event(event)?;
    validate_freshness(verified.created_at, request.freshness)?;
    validate_replay(request.replay, verified.event_id)?;
    let context = verify_authentication_context(&verified, request.authentication)?;
    let profile_identity = profile_identity(context.profile);
    let implementation_identity = tagged_blake3(IMPLEMENTATION_IDENTITY_DOMAIN, &[IMPLEMENTATION_REVISION.as_bytes()]);

    Ok(VerifiedNostrCredential {
        public_key: verified.public_key,
        event_id: verified.event_id,
        profile: context.profile,
        audience: context.audience.to_owned(),
        purpose: context.purpose.to_owned(),
        request_reference: context.request_reference,
        freshness: FreshnessDisposition::Fresh,
        replay: ReplayDisposition::Fresh,
        adapter_profile_identity: profile_identity,
        implementation_identity,
    })
}

struct VerifiedContext<'a> {
    profile: AuthenticationProfile,
    audience: &'a str,
    purpose: &'a str,
    request_reference: AdapterBlake3,
}

fn verify_authentication_context<'a>(
    event: &VerifiedNostrEvent,
    expectation: AuthenticationExpectation<'a>,
) -> Result<VerifiedContext<'a>, VerificationIssue> {
    match expectation {
        AuthenticationExpectation::Nip42Relay {
            relay_url,
            challenge,
            expected_audience,
            expected_purpose,
        } => verify_nip42(event, relay_url, challenge, expected_audience, expected_purpose),
        AuthenticationExpectation::Nip98Http {
            absolute_url,
            method,
            payload_sha256,
            expected_audience,
            expected_purpose,
        } => verify_nip98(event, absolute_url, method, payload_sha256, expected_audience, expected_purpose),
        AuthenticationExpectation::ApplicationV1 {
            domain,
            audience,
            purpose,
            challenge,
            request_identity,
            event_kind,
        } => verify_application(event, domain, audience, purpose, challenge, request_identity, event_kind),
    }
}

fn verify_nip42<'a>(
    event: &VerifiedNostrEvent,
    relay_url: &'a str,
    challenge: &str,
    expected_audience: &str,
    expected_purpose: &str,
) -> Result<VerifiedContext<'a>, VerificationIssue> {
    if !has_absolute_url_with_scheme(relay_url, &["ws://", "wss://"]) {
        return Err(VerificationIssue::InvalidAuthenticationContext);
    }
    validate_kind_and_content(event, NIP42_EVENT_KIND)?;
    require_tag(&event.tags, "relay", relay_url)?;
    require_tag(&event.tags, "challenge", challenge)?;
    if expected_audience != relay_url {
        return Err(VerificationIssue::AudienceMismatch);
    }
    if expected_purpose != NIP42_PURPOSE {
        return Err(VerificationIssue::PurposeMismatch);
    }
    let request_reference =
        tagged_blake3(REQUEST_REFERENCE_DOMAIN, &[b"nip42-relay", relay_url.as_bytes(), challenge.as_bytes()]);
    Ok(VerifiedContext {
        profile: AuthenticationProfile::Nip42Relay,
        audience: relay_url,
        purpose: NIP42_PURPOSE,
        request_reference,
    })
}

fn verify_nip98<'a>(
    event: &VerifiedNostrEvent,
    absolute_url: &'a str,
    method: &str,
    payload_sha256: Option<NostrEventId>,
    expected_audience: &str,
    expected_purpose: &str,
) -> Result<VerifiedContext<'a>, VerificationIssue> {
    if !has_absolute_url_with_scheme(absolute_url, &["http://", "https://"]) || !is_http_method_token(method) {
        return Err(VerificationIssue::InvalidAuthenticationContext);
    }
    validate_kind_and_content(event, NIP98_EVENT_KIND)?;
    require_tag(&event.tags, "u", absolute_url)?;
    require_tag(&event.tags, "method", method)?;
    match payload_sha256 {
        Some(payload) => require_tag(&event.tags, "payload", &payload.to_hex())?,
        None => require_tag_absent(&event.tags, "payload")?,
    }
    if expected_audience != absolute_url {
        return Err(VerificationIssue::AudienceMismatch);
    }
    if expected_purpose != NIP98_PURPOSE {
        return Err(VerificationIssue::PurposeMismatch);
    }
    let payload_text = payload_sha256.map_or_else(String::new, NostrEventId::to_hex);
    let request_reference = tagged_blake3(REQUEST_REFERENCE_DOMAIN, &[
        b"nip98-http",
        absolute_url.as_bytes(),
        method.as_bytes(),
        payload_text.as_bytes(),
    ]);
    Ok(VerifiedContext {
        profile: AuthenticationProfile::Nip98Http,
        audience: absolute_url,
        purpose: NIP98_PURPOSE,
        request_reference,
    })
}

fn verify_application<'a>(
    event: &VerifiedNostrEvent,
    domain: &str,
    audience: &'a str,
    purpose: &'a str,
    challenge: &str,
    request_identity: &str,
    event_kind: u32,
) -> Result<VerifiedContext<'a>, VerificationIssue> {
    if event_kind == NIP42_EVENT_KIND || event_kind == NIP98_EVENT_KIND {
        return Err(VerificationIssue::ProfileKindMismatch);
    }
    validate_kind_and_content(event, event_kind)?;
    require_tag(&event.tags, "domain", domain)?;
    require_tag(&event.tags, "audience", audience)?;
    require_tag(&event.tags, "purpose", purpose)?;
    require_tag(&event.tags, "challenge", challenge)?;
    require_tag(&event.tags, "request", request_identity)?;
    let event_kind_bytes = event_kind.to_le_bytes();
    let request_reference = tagged_blake3(REQUEST_REFERENCE_DOMAIN, &[
        APPLICATION_PURPOSE_PROFILE.as_bytes(),
        domain.as_bytes(),
        audience.as_bytes(),
        purpose.as_bytes(),
        challenge.as_bytes(),
        request_identity.as_bytes(),
        &event_kind_bytes,
    ]);
    Ok(VerifiedContext {
        profile: AuthenticationProfile::ApplicationV1,
        audience,
        purpose,
        request_reference,
    })
}

const fn validate_kind_and_content(event: &VerifiedNostrEvent, expected_kind: u32) -> Result<(), VerificationIssue> {
    if event.kind != expected_kind {
        return Err(VerificationIssue::ProfileKindMismatch);
    }
    if !event.content.is_empty() {
        return Err(VerificationIssue::ProfileContentMismatch);
    }
    Ok(())
}

fn validate_freshness(created_at: u64, policy: FreshnessExpectation) -> Result<(), VerificationIssue> {
    if policy.max_age_seconds == 0 {
        return Err(VerificationIssue::InvalidFreshnessPolicy);
    }
    let newest = policy
        .observed_now
        .checked_add(policy.max_future_skew_seconds)
        .ok_or(VerificationIssue::InvalidFreshnessPolicy)?;
    if created_at > newest {
        return Err(VerificationIssue::EventFromFuture);
    }
    let oldest = policy.observed_now.saturating_sub(policy.max_age_seconds);
    if created_at < oldest {
        return Err(VerificationIssue::EventStale);
    }
    Ok(())
}

fn validate_replay(observation: ReplayObservation, verified_event_id: NostrEventId) -> Result<(), VerificationIssue> {
    match observation {
        ReplayObservation::Fresh { event_id } if event_id == verified_event_id => Ok(()),
        ReplayObservation::Fresh { .. } | ReplayObservation::Replayed { .. } | ReplayObservation::Unavailable => {
            Err(VerificationIssue::ReplayDenied)
        }
    }
}

fn require_tag(tags: &[Vec<String>], name: &str, expected: &str) -> Result<(), VerificationIssue> {
    let value = find_unique_tag(tags, name)?.ok_or(VerificationIssue::MissingTag)?;
    if value == expected {
        Ok(())
    } else {
        Err(VerificationIssue::TagMismatch)
    }
}

fn require_tag_absent(tags: &[Vec<String>], name: &str) -> Result<(), VerificationIssue> {
    if find_unique_tag(tags, name)?.is_none() {
        Ok(())
    } else {
        Err(VerificationIssue::TagMismatch)
    }
}

fn find_unique_tag<'a>(tags: &'a [Vec<String>], name: &str) -> Result<Option<&'a str>, VerificationIssue> {
    let mut matching = tags.iter().filter(|tag| tag.first().is_some_and(|candidate| candidate.as_str() == name));
    let Some(tag) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(VerificationIssue::DuplicateTag);
    }
    let [tag_name, value] = tag.as_slice() else {
        return Err(VerificationIssue::TagMismatch);
    };
    assert_eq!(tag_name, name, "selected tag name must match the filter");
    Ok(Some(value))
}

fn has_absolute_url_with_scheme(value: &str, schemes: &[&str]) -> bool {
    let Some(remainder) = schemes.iter().find_map(|scheme| value.strip_prefix(scheme)) else {
        return false;
    };
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or("");
    !authority.is_empty() && !authority.contains('@') && !value.contains('#') && !value.chars().any(char::is_whitespace)
}

fn is_http_method_token(value: &str) -> bool {
    const HTTP_TOKEN_PUNCTUATION: &[u8] = b"!#$%&'*+-.^_`|~";
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || HTTP_TOKEN_PUNCTUATION.contains(&byte))
}

fn validate_context_field_limits(expectation: AuthenticationExpectation<'_>) -> Result<(), VerificationIssue> {
    let fields: &[&str] = match expectation {
        AuthenticationExpectation::Nip42Relay {
            relay_url,
            challenge,
            expected_audience,
            expected_purpose,
        } => &[relay_url, challenge, expected_audience, expected_purpose],
        AuthenticationExpectation::Nip98Http {
            absolute_url,
            method,
            expected_audience,
            expected_purpose,
            ..
        } => &[absolute_url, method, expected_audience, expected_purpose],
        AuthenticationExpectation::ApplicationV1 {
            domain,
            audience,
            purpose,
            challenge,
            request_identity,
            ..
        } => &[domain, audience, purpose, challenge, request_identity],
    };
    if fields.iter().any(|field| field.is_empty() || field.len() > MAX_CONTEXT_FIELD_BYTES) {
        Err(VerificationIssue::EventBoundsExceeded)
    } else {
        Ok(())
    }
}

fn profile_identity(profile: AuthenticationProfile) -> AdapterBlake3 {
    let profile_name = match profile {
        AuthenticationProfile::Nip42Relay => "nip42-relay",
        AuthenticationProfile::Nip98Http => "nip98-http",
        AuthenticationProfile::ApplicationV1 => APPLICATION_PURPOSE_PROFILE,
    };
    tagged_blake3(PROFILE_IDENTITY_DOMAIN, &[profile_name.as_bytes()])
}

fn tagged_blake3(domain: &str, values: &[&[u8]]) -> AdapterBlake3 {
    let mut hasher = blake3::Hasher::new();
    update_framed(&mut hasher, domain.as_bytes());
    for value in values {
        update_framed(&mut hasher, value);
    }
    AdapterBlake3::new(*hasher.finalize().as_bytes())
}

fn update_framed(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded adapter input length must fit u64");
    hasher.update(&length.to_le_bytes());
    hasher.update(value);
}
