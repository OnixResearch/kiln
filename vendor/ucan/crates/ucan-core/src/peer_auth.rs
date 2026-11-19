//! Optional deterministic authenticated-message framing.
//!
//! This module has no transport, clock, key-store, replay-store, filesystem,
//! process, or network effects. Callers supply every observation as a value.

use alloc::string::String;
use core::fmt;

use hmac::Hmac;
use hmac::Mac;
use sha2::Digest;
use sha2::Sha256;
use zeroize::Zeroize;

/// Pinned bounded Celld comparison revision.
pub const CELLD_REFERENCE_REVISION: &str = "c978a2590e875b003ecd860a86cb7551287eba67";
/// Pinned selected Celld peer-auth source blob.
pub const CELLD_REFERENCE_SOURCE_BLOB: &str = "40d6ac4613d293a148e7222e192153eaeaeb29db";
/// License declared by the pinned Celld repository and selected source.
pub const CELLD_REFERENCE_LICENSE: &str = "Apache-2.0";
/// Default UCAN peer-auth framing version.
pub const PEER_AUTH_PROTOCOL_VERSION: u16 = 1;
/// Required shared authentication-key length.
pub const AUTHENTICATION_KEY_BYTES: usize = 32;
/// Required replay nonce length.
pub const REPLAY_NONCE_BYTES: usize = 16;
/// HMAC-SHA-256 tag length.
pub const AUTHENTICATOR_BYTES: usize = 32;
/// SHA-256 body digest length.
pub const BODY_DIGEST_BYTES: usize = 32;
/// BLAKE3 identity length.
pub const MESSAGE_IDENTITY_BYTES: usize = 32;
/// Largest admitted opaque routing context.
pub const MAX_ROUTING_CONTEXT_BYTES: usize = 4_096;
/// Largest admitted message body.
pub const MAX_AUTHENTICATED_BODY_BYTES: usize = 1_048_576;

const AUTHENTICATOR_DOMAIN: &[u8] = b"ucan-peer-auth.authenticator.v1";
const MESSAGE_IDENTITY_DOMAIN: &[u8] = b"ucan-peer-auth.message-identity.v1";
const AUTHENTICATOR_REFERENCE_DOMAIN: &[u8] = b"ucan-peer-auth.authenticator-reference.v1";
const IMPLEMENTATION_IDENTITY_DOMAIN: &[u8] = b"ucan-peer-auth.implementation-identity.v1";
const IMPLEMENTATION_REVISION: &[u8] =
    b"ucan-core@0.1.0|peer-auth-v1|blake3@1.8.5|hmac@0.12.1|sha2@0.10.9|zeroize@1.9.0";
const HEX_CHARS_PER_BYTE: usize = 2;
const NIBBLE_BITS: u32 = 4;
const LOW_NIBBLE_MASK: u8 = 0x0f;
const HEX_RADIX: usize = 16;
const REDACTED: &str = "<redacted>";
const NON_CLAIMS: &[&str] = &[
    "authenticated-message validation does not grant a UCAN capability or authorization",
    "authenticated-message validation does not authenticate an end user or resolve a subject",
    "transport, TLS, key custody, clocks, replay storage, membership, and routing remain caller-owned",
    "the Celld reference supplies bounded comparison constraints, not parity or equivalence",
];

const _: () = assert!(AUTHENTICATION_KEY_BYTES > 0);
const _: () = assert!(REPLAY_NONCE_BYTES > 0);
const _: () = assert!(MAX_ROUTING_CONTEXT_BYTES > 0);
const _: () = assert!(MAX_AUTHENTICATED_BODY_BYTES > 0);

/// Stable validation issue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerAuthIssue {
    /// Protocol version zero is not admitted.
    InvalidProtocolVersion,
    /// Message protocol version differs from the caller-negotiated version.
    ProtocolVersionMismatch,
    /// Opaque routing context is empty.
    EmptyRoutingContext,
    /// Opaque routing context exceeds its fixed limit.
    RoutingContextTooLarge,
    /// Message body is empty.
    EmptyMessageBody,
    /// Message body exceeds its fixed limit.
    MessageBodyTooLarge,
    /// Message routing context differs from the caller expectation.
    RoutingContextMismatch,
    /// Supplied body bytes differ from the signed body digest.
    BodyDigestMismatch,
    /// HMAC-SHA-256 authentication failed.
    AuthenticatorMismatch,
    /// Message timestamp is older than the caller clock bound.
    MessageStale,
    /// Message timestamp is newer than the caller clock bound.
    MessageFromFuture,
    /// Replay observation denied this exact nonce or was unavailable.
    ReplayDenied,
    /// Input attempted secret, capability, subject, or transport promotion.
    BoundaryPromotion,
    /// A digest was supplied for the wrong algorithm role.
    DigestRoleMismatch,
}

impl PeerAuthIssue {
    /// Return the stable machine issue code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidProtocolVersion => "peer-auth-version-invalid",
            Self::ProtocolVersionMismatch => "peer-auth-version-mismatch",
            Self::EmptyRoutingContext => "peer-auth-context-empty",
            Self::RoutingContextTooLarge => "peer-auth-context-too-large",
            Self::EmptyMessageBody => "peer-auth-body-empty",
            Self::MessageBodyTooLarge => "peer-auth-body-too-large",
            Self::RoutingContextMismatch => "peer-auth-context-mismatch",
            Self::BodyDigestMismatch => "peer-auth-body-digest-mismatch",
            Self::AuthenticatorMismatch => "peer-auth-authenticator-mismatch",
            Self::MessageStale => "peer-auth-message-stale",
            Self::MessageFromFuture => "peer-auth-message-from-future",
            Self::ReplayDenied => "peer-auth-replay-denied",
            Self::BoundaryPromotion => "peer-auth-boundary-promotion",
            Self::DigestRoleMismatch => "peer-auth-digest-role-mismatch",
        }
    }
}

/// Non-zero negotiated protocol version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolVersion(u16);

impl ProtocolVersion {
    /// Construct a non-zero version.
    ///
    /// # Errors
    ///
    /// Returns [`PeerAuthIssue::InvalidProtocolVersion`] for zero.
    pub const fn new(value: u16) -> Result<Self, PeerAuthIssue> {
        if value == 0 {
            Err(PeerAuthIssue::InvalidProtocolVersion)
        } else {
            Ok(Self(value))
        }
    }

    /// Return the numeric protocol version.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Opaque fixed-size shared authentication key.
pub struct AuthenticationKey([u8; AUTHENTICATION_KEY_BYTES]);

impl AuthenticationKey {
    /// Construct from caller-owned key bytes.
    #[must_use]
    pub const fn new(bytes: [u8; AUTHENTICATION_KEY_BYTES]) -> Self {
        Self(bytes)
    }

    const fn as_bytes(&self) -> &[u8; AUTHENTICATION_KEY_BYTES] {
        &self.0
    }
}

impl Drop for AuthenticationKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for AuthenticationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("AuthenticationKey").field("secret", &REDACTED).finish()
    }
}

/// Caller-generated fixed-size replay nonce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayNonce([u8; REPLAY_NONCE_BYTES]);

impl ReplayNonce {
    /// Construct from caller-generated bytes.
    #[must_use]
    pub const fn new(bytes: [u8; REPLAY_NONCE_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return nonce bytes for caller-owned replay storage and wire encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; REPLAY_NONCE_BYTES] {
        &self.0
    }
}

/// SHA-256 digest of exact message body bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodySha256([u8; BODY_DIGEST_BYTES]);

impl BodySha256 {
    /// Construct from wire-decoded digest bytes.
    #[must_use]
    pub const fn new(bytes: [u8; BODY_DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return body digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; BODY_DIGEST_BYTES] {
        &self.0
    }
}

/// HMAC-SHA-256 authentication tag.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AuthenticatorTag([u8; AUTHENTICATOR_BYTES]);

impl AuthenticatorTag {
    /// Construct from wire-decoded tag bytes.
    #[must_use]
    pub const fn new(bytes: [u8; AUTHENTICATOR_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return tag bytes for wire encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; AUTHENTICATOR_BYTES] {
        &self.0
    }
}

impl fmt::Debug for AuthenticatorTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AuthenticatorTag(<redacted>)")
    }
}

/// Domain-separated BLAKE3 message identity or reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageBlake3([u8; MESSAGE_IDENTITY_BYTES]);

impl MessageBlake3 {
    /// Construct from wire-decoded identity bytes.
    #[must_use]
    pub const fn new(bytes: [u8; MESSAGE_IDENTITY_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; MESSAGE_IDENTITY_BYTES] {
        &self.0
    }

    /// Return lowercase hexadecimal identity text.
    #[must_use]
    pub fn to_hex(self) -> String {
        encode_lower_hex(&self.0)
    }
}

/// Closed digest algorithm role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDigestRole {
    /// SHA-256 digest of body bytes.
    BodySha256,
    /// HMAC-SHA-256 authentication tag.
    AuthenticatorHmacSha256,
    /// Domain-separated BLAKE3 message identity.
    MessageBlake3,
}

/// Structural wire digest with its declared role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireMessageDigest {
    /// Declared algorithm role.
    pub role: MessageDigestRole,
    /// Fixed digest bytes.
    pub bytes: [u8; MESSAGE_IDENTITY_BYTES],
}

/// Validate one structural digest role.
///
/// # Errors
///
/// Returns [`PeerAuthIssue::DigestRoleMismatch`] when the role differs.
pub fn validate_digest_role(
    digest: WireMessageDigest,
    expected: MessageDigestRole,
) -> Result<[u8; MESSAGE_IDENTITY_BYTES], PeerAuthIssue> {
    if digest.role == expected {
        Ok(digest.bytes)
    } else {
        Err(PeerAuthIssue::DigestRoleMismatch)
    }
}

/// Explicit boundary fact that must never promote message authentication.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MessageBoundary {
    /// No secret or authority promotion is requested.
    #[default]
    None,
    /// Message metadata contains a raw shared secret or private credential.
    ContainsSecretMaterial,
    /// Message metadata claims a UCAN capability or authorization.
    PromotesCapability,
    /// Message metadata claims an end-user or product subject.
    PromotesSubject,
    /// Message metadata claims built-in transport or TLS authority.
    PromotesTransportAuthority,
}

/// Unsigned message facts supplied by a caller-owned shell.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct MessageDraft<'a> {
    /// Protocol version selected by the sender.
    pub protocol_version: ProtocolVersion,
    /// Sender-observed Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Caller-generated replay nonce.
    pub nonce: ReplayNonce,
    /// Opaque transport-owned routing context.
    pub routing_context: &'a [u8],
    /// Exact message body bytes.
    pub body: &'a [u8],
    /// Explicit non-promotion boundary.
    pub boundary: MessageBoundary,
}

impl fmt::Debug for MessageDraft<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageDraft")
            .field("protocol_version", &self.protocol_version)
            .field("timestamp_ms", &self.timestamp_ms)
            .field("nonce", &self.nonce)
            .field("routing_context", &REDACTED)
            .field("routing_context_bytes", &self.routing_context.len())
            .field("body", &REDACTED)
            .field("body_bytes", &self.body.len())
            .field("boundary", &self.boundary)
            .finish()
    }
}

/// Signed wire frame with borrowed body and routing context.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AuthenticatedMessageFrame<'a> {
    protocol_version: ProtocolVersion,
    timestamp_ms: u64,
    nonce: ReplayNonce,
    routing_context: &'a [u8],
    body: &'a [u8],
    body_sha256: BodySha256,
    authenticator: AuthenticatorTag,
    boundary: MessageBoundary,
}

impl<'a> AuthenticatedMessageFrame<'a> {
    /// Construct a frame from wire-decoded fields.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn from_wire_parts(
        protocol_version: ProtocolVersion,
        timestamp_ms: u64,
        nonce: ReplayNonce,
        routing_context: &'a [u8],
        body: &'a [u8],
        body_sha256: BodySha256,
        authenticator: AuthenticatorTag,
        boundary: MessageBoundary,
    ) -> Self {
        Self {
            protocol_version,
            timestamp_ms,
            nonce,
            routing_context,
            body,
            body_sha256,
            authenticator,
            boundary,
        }
    }

    /// Return the signed protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    /// Return the signed timestamp.
    #[must_use]
    pub const fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    /// Return the signed replay nonce.
    #[must_use]
    pub const fn nonce(&self) -> ReplayNonce {
        self.nonce
    }

    /// Return caller-owned routing context bytes.
    #[must_use]
    pub const fn routing_context(&self) -> &'a [u8] {
        self.routing_context
    }

    /// Return exact body bytes.
    #[must_use]
    pub const fn body(&self) -> &'a [u8] {
        self.body
    }

    /// Return the claimed body digest.
    #[must_use]
    pub const fn body_sha256(&self) -> BodySha256 {
        self.body_sha256
    }

    /// Return the authentication tag for wire encoding.
    #[must_use]
    pub const fn authenticator(&self) -> AuthenticatorTag {
        self.authenticator
    }
}

impl fmt::Debug for AuthenticatedMessageFrame<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedMessageFrame")
            .field("protocol_version", &self.protocol_version)
            .field("timestamp_ms", &self.timestamp_ms)
            .field("nonce", &self.nonce)
            .field("routing_context", &REDACTED)
            .field("routing_context_bytes", &self.routing_context.len())
            .field("body", &REDACTED)
            .field("body_bytes", &self.body.len())
            .field("body_sha256", &self.body_sha256)
            .field("authenticator", &self.authenticator)
            .field("boundary", &self.boundary)
            .finish()
    }
}

/// Caller-owned replay-store observation for one exact nonce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayObservation {
    /// Caller reports this exact nonce as unused.
    Fresh { nonce: ReplayNonce },
    /// Caller reports this exact nonce as already used.
    Replayed { nonce: ReplayNonce },
    /// Caller could not obtain a fail-closed replay observation.
    Unavailable,
}

/// Explicit validation policy and observations.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct MessageExpectation<'a> {
    /// Caller-negotiated protocol version.
    pub protocol_version: ProtocolVersion,
    /// Exact caller-owned routing context.
    pub routing_context: &'a [u8],
    /// Explicit observed Unix time in milliseconds.
    pub observed_now_ms: u64,
    /// Largest accepted past or future clock difference.
    pub max_clock_skew_ms: u64,
    /// Caller-owned replay observation.
    pub replay: ReplayObservation,
}

impl fmt::Debug for MessageExpectation<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageExpectation")
            .field("protocol_version", &self.protocol_version)
            .field("routing_context", &REDACTED)
            .field("routing_context_bytes", &self.routing_context.len())
            .field("observed_now_ms", &self.observed_now_ms)
            .field("max_clock_skew_ms", &self.max_clock_skew_ms)
            .field("replay", &self.replay)
            .finish()
    }
}

/// Successful clock-bound disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageFreshness {
    /// Message timestamp satisfied the caller clock bound.
    WithinClockBound,
}

/// Successful replay disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageReplay {
    /// Caller admitted the exact message nonce as unused.
    Fresh,
}

/// Opaque successful authenticated-message verdict.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedAuthenticatedMessage {
    protocol_version: ProtocolVersion,
    message_identity: MessageBlake3,
    body_bound_signature_reference: MessageBlake3,
    freshness: MessageFreshness,
    replay: MessageReplay,
    implementation_identity: MessageBlake3,
}

impl VerifiedAuthenticatedMessage {
    /// Return the negotiated protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    /// Return the authenticated envelope BLAKE3 identity.
    #[must_use]
    pub const fn message_identity(&self) -> MessageBlake3 {
        self.message_identity
    }

    /// Return a BLAKE3 reference to the checked body-bound HMAC tag.
    #[must_use]
    pub const fn body_bound_signature_reference(&self) -> MessageBlake3 {
        self.body_bound_signature_reference
    }

    /// Return the successful clock disposition.
    #[must_use]
    pub const fn freshness(&self) -> MessageFreshness {
        self.freshness
    }

    /// Return the successful replay disposition.
    #[must_use]
    pub const fn replay(&self) -> MessageReplay {
        self.replay
    }

    /// Return the implementation BLAKE3 identity.
    #[must_use]
    pub const fn implementation_identity(&self) -> MessageBlake3 {
        self.implementation_identity
    }

    /// Return required authority and integration non-claims.
    #[must_use]
    pub const fn non_claims(&self) -> &'static [&'static str] {
        NON_CLAIMS
    }
}

impl fmt::Debug for VerifiedAuthenticatedMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedAuthenticatedMessage")
            .field("protocol_version", &self.protocol_version)
            .field("message_identity", &self.message_identity)
            .field("body_bound_signature_reference", &self.body_bound_signature_reference)
            .field("freshness", &self.freshness)
            .field("replay", &self.replay)
            .field("implementation_identity", &self.implementation_identity)
            .finish()
    }
}

/// Sign one bounded message using caller-supplied values only.
///
/// # Errors
///
/// Returns a deterministic issue for empty, oversized, or promoted input.
// r[impl ucan.peer_auth.framing]
pub fn authenticate_message<'a>(
    draft: MessageDraft<'a>,
    key: &AuthenticationKey,
) -> Result<AuthenticatedMessageFrame<'a>, PeerAuthIssue> {
    validate_message_shape(draft.routing_context, draft.body, draft.boundary)?;
    let body_sha256 = BodySha256(compute_sha256(draft.body));
    let authenticator = compute_authenticator(
        key,
        draft.protocol_version,
        draft.timestamp_ms,
        draft.nonce,
        draft.routing_context,
        body_sha256,
    );
    Ok(AuthenticatedMessageFrame {
        protocol_version: draft.protocol_version,
        timestamp_ms: draft.timestamp_ms,
        nonce: draft.nonce,
        routing_context: draft.routing_context,
        body: draft.body,
        body_sha256,
        authenticator,
        boundary: draft.boundary,
    })
}

/// Verify protocol version, exact context and body, HMAC, time, and replay.
///
/// # Errors
///
/// Returns a deterministic issue and constructs no verdict unless all checks
/// pass.
// r[impl ucan.peer_auth.validation]
// r[impl ucan.peer_auth.verdict]
pub fn verify_authenticated_message(
    frame: &AuthenticatedMessageFrame<'_>,
    key: &AuthenticationKey,
    expectation: MessageExpectation<'_>,
) -> Result<VerifiedAuthenticatedMessage, PeerAuthIssue> {
    validate_message_shape(frame.routing_context, frame.body, frame.boundary)?;
    validate_routing_context(expectation.routing_context)?;
    if frame.protocol_version != expectation.protocol_version {
        return Err(PeerAuthIssue::ProtocolVersionMismatch);
    }
    if frame.routing_context != expectation.routing_context {
        return Err(PeerAuthIssue::RoutingContextMismatch);
    }
    let body_sha256 = BodySha256(compute_sha256(frame.body));
    if body_sha256 != frame.body_sha256 {
        return Err(PeerAuthIssue::BodyDigestMismatch);
    }
    verify_authenticator(frame, key)?;
    validate_clock_bound(frame.timestamp_ms, expectation.observed_now_ms, expectation.max_clock_skew_ms)?;
    validate_replay(frame.nonce, expectation.replay)?;

    let message_identity = message_identity(frame);
    let body_bound_signature_reference =
        tagged_blake3(AUTHENTICATOR_REFERENCE_DOMAIN, &[frame.authenticator.as_bytes()]);
    let implementation_identity = tagged_blake3(IMPLEMENTATION_IDENTITY_DOMAIN, &[IMPLEMENTATION_REVISION]);
    Ok(VerifiedAuthenticatedMessage {
        protocol_version: expectation.protocol_version,
        message_identity,
        body_bound_signature_reference,
        freshness: MessageFreshness::WithinClockBound,
        replay: MessageReplay::Fresh,
        implementation_identity,
    })
}

// r[impl ucan.peer_auth.boundary]
const fn validate_boundary(boundary: MessageBoundary) -> Result<(), PeerAuthIssue> {
    if matches!(boundary, MessageBoundary::None) {
        Ok(())
    } else {
        Err(PeerAuthIssue::BoundaryPromotion)
    }
}

fn validate_message_shape(routing_context: &[u8], body: &[u8], boundary: MessageBoundary) -> Result<(), PeerAuthIssue> {
    validate_boundary(boundary)?;
    validate_routing_context(routing_context)?;
    if body.is_empty() {
        return Err(PeerAuthIssue::EmptyMessageBody);
    }
    if body.len() > MAX_AUTHENTICATED_BODY_BYTES {
        return Err(PeerAuthIssue::MessageBodyTooLarge);
    }
    Ok(())
}

const fn validate_routing_context(routing_context: &[u8]) -> Result<(), PeerAuthIssue> {
    if routing_context.is_empty() {
        return Err(PeerAuthIssue::EmptyRoutingContext);
    }
    if routing_context.len() > MAX_ROUTING_CONTEXT_BYTES {
        return Err(PeerAuthIssue::RoutingContextTooLarge);
    }
    Ok(())
}

const fn validate_clock_bound(
    timestamp_ms: u64,
    observed_now_ms: u64,
    max_clock_skew_ms: u64,
) -> Result<(), PeerAuthIssue> {
    if timestamp_ms > observed_now_ms {
        if timestamp_ms - observed_now_ms > max_clock_skew_ms {
            return Err(PeerAuthIssue::MessageFromFuture);
        }
    } else if observed_now_ms - timestamp_ms > max_clock_skew_ms {
        return Err(PeerAuthIssue::MessageStale);
    }
    Ok(())
}

fn validate_replay(nonce: ReplayNonce, observation: ReplayObservation) -> Result<(), PeerAuthIssue> {
    match observation {
        ReplayObservation::Fresh { nonce: observed } if nonce.0 == observed.0 => Ok(()),
        ReplayObservation::Fresh { .. } | ReplayObservation::Replayed { .. } | ReplayObservation::Unavailable => {
            Err(PeerAuthIssue::ReplayDenied)
        }
    }
}

fn verify_authenticator(frame: &AuthenticatedMessageFrame<'_>, key: &AuthenticationKey) -> Result<(), PeerAuthIssue> {
    let mac = canonical_mac(
        key.as_bytes(),
        frame.protocol_version,
        frame.timestamp_ms,
        frame.nonce,
        frame.routing_context,
        frame.body_sha256,
    );
    mac.verify_slice(frame.authenticator.as_bytes())
        .map_err(|_error| PeerAuthIssue::AuthenticatorMismatch)
}

fn compute_authenticator(
    key: &AuthenticationKey,
    protocol_version: ProtocolVersion,
    timestamp_ms: u64,
    nonce: ReplayNonce,
    routing_context: &[u8],
    body_sha256: BodySha256,
) -> AuthenticatorTag {
    let mac = canonical_mac(key.as_bytes(), protocol_version, timestamp_ms, nonce, routing_context, body_sha256);
    AuthenticatorTag(mac.finalize().into_bytes().into())
}

fn canonical_mac(
    key: &[u8],
    protocol_version: ProtocolVersion,
    timestamp_ms: u64,
    nonce: ReplayNonce,
    routing_context: &[u8],
    body_sha256: BodySha256,
) -> Hmac<Sha256> {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC-SHA-256 accepts the fixed authentication key length");
    update_framed_mac(&mut mac, AUTHENTICATOR_DOMAIN);
    update_framed_mac(&mut mac, &protocol_version.get().to_be_bytes());
    update_framed_mac(&mut mac, &timestamp_ms.to_be_bytes());
    update_framed_mac(&mut mac, nonce.as_bytes());
    update_framed_mac(&mut mac, routing_context);
    update_framed_mac(&mut mac, body_sha256.as_bytes());
    mac
}

fn update_framed_mac(mac: &mut Hmac<Sha256>, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded message input length must fit u64");
    mac.update(&length.to_be_bytes());
    mac.update(value);
}

fn compute_sha256(value: &[u8]) -> [u8; BODY_DIGEST_BYTES] {
    Sha256::digest(value).into()
}

fn message_identity(frame: &AuthenticatedMessageFrame<'_>) -> MessageBlake3 {
    tagged_blake3(MESSAGE_IDENTITY_DOMAIN, &[
        &frame.protocol_version.get().to_be_bytes(),
        &frame.timestamp_ms.to_be_bytes(),
        frame.nonce.as_bytes(),
        frame.routing_context,
        frame.body_sha256.as_bytes(),
        frame.authenticator.as_bytes(),
    ])
}

fn tagged_blake3(domain: &[u8], values: &[&[u8]]) -> MessageBlake3 {
    let mut hasher = blake3::Hasher::new();
    update_framed_blake3(&mut hasher, domain);
    for value in values {
        update_framed_blake3(&mut hasher, value);
    }
    MessageBlake3(*hasher.finalize().as_bytes())
}

fn update_framed_blake3(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded message input length must fit u64");
    hasher.update(&length.to_be_bytes());
    hasher.update(value);
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; HEX_RADIX] = b"0123456789abcdef";
    let output_capacity = bytes.len().saturating_mul(HEX_CHARS_PER_BYTE);
    let mut output = String::with_capacity(output_capacity);
    for byte in bytes {
        let high = usize::from(*byte >> NIBBLE_BITS);
        let low = usize::from(*byte & LOW_NIBBLE_MASK);
        output.push(char::from(HEX[high]));
        output.push(char::from(HEX[low]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT_SHA1_HEX_LENGTH: usize = 40;
    const TEST_KEY_BYTE: u8 = 0xa5;
    const TEST_KEY: [u8; AUTHENTICATION_KEY_BYTES] = [TEST_KEY_BYTE; AUTHENTICATION_KEY_BYTES];
    const TEST_NONCE_BYTE: u8 = 0x5a;
    const TEST_NONCE: ReplayNonce = ReplayNonce::new([TEST_NONCE_BYTE; REPLAY_NONCE_BYTES]);
    const OTHER_NONCE_BYTE: u8 = 0x33;
    const OTHER_NONCE: ReplayNonce = ReplayNonce::new([OTHER_NONCE_BYTE; REPLAY_NONCE_BYTES]);
    const TEST_TIMESTAMP_MS: u64 = 1_700_000_000_000;
    const CLOCK_SKEW_MS: u64 = 30_000;
    const OUT_OF_BOUND_DELTA_MS: u64 = CLOCK_SKEW_MS + 1;
    const TEST_CONTEXT: &[u8] = b"POST\n/peer/message\nnode-a\nnode-b";
    const OTHER_CONTEXT: &[u8] = b"POST\n/peer/message\nnode-a\nnode-c";
    const TEST_BODY: &[u8] = b"bounded authenticated body";
    const ALTERED_BODY: &[u8] = b"altered authenticated body";
    const RFC_4231_KEY_BYTES: usize = 20;
    const RFC_4231_KEY_BYTE: u8 = 0x0b;
    const RFC_4231_KEY: [u8; RFC_4231_KEY_BYTES] = [RFC_4231_KEY_BYTE; RFC_4231_KEY_BYTES];
    const RFC_4231_DATA: &[u8] = b"Hi There";
    const RFC_4231_HMAC_SHA256: [u8; AUTHENTICATOR_BYTES] = [
        0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b, 0xf1, 0x2b, 0x88, 0x1d,
        0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c, 0x2e, 0x32, 0xcf, 0xf7,
    ];

    fn version() -> ProtocolVersion {
        ProtocolVersion::new(PEER_AUTH_PROTOCOL_VERSION).expect("test protocol version must be non-zero")
    }

    fn key() -> AuthenticationKey {
        AuthenticationKey::new(TEST_KEY)
    }

    fn draft() -> MessageDraft<'static> {
        MessageDraft {
            protocol_version: version(),
            timestamp_ms: TEST_TIMESTAMP_MS,
            nonce: TEST_NONCE,
            routing_context: TEST_CONTEXT,
            body: TEST_BODY,
            boundary: MessageBoundary::None,
        }
    }

    fn expectation(replay: ReplayObservation) -> MessageExpectation<'static> {
        MessageExpectation {
            protocol_version: version(),
            routing_context: TEST_CONTEXT,
            observed_now_ms: TEST_TIMESTAMP_MS,
            max_clock_skew_ms: CLOCK_SKEW_MS,
            replay,
        }
    }

    // r[verify ucan.peer_auth.reference]
    // r[verify ucan.peer_auth.verification]
    #[test]
    fn rfc_4231_hmac_sha256_vector_and_matching_frame_pass() {
        let mac = <Hmac<Sha256> as Mac>::new_from_slice(&RFC_4231_KEY)
            .expect("RFC key must be admitted by HMAC-SHA-256")
            .chain_update(RFC_4231_DATA)
            .finalize()
            .into_bytes();
        let frame = authenticate_message(draft(), &key()).expect("bounded message must authenticate");
        let verified =
            verify_authenticated_message(&frame, &key(), expectation(ReplayObservation::Fresh { nonce: TEST_NONCE }))
                .expect("matching authenticated message must verify");

        assert_eq!(<[u8; AUTHENTICATOR_BYTES]>::from(mac), RFC_4231_HMAC_SHA256);
        assert_eq!(verified.protocol_version(), version());
        assert_eq!(verified.freshness(), MessageFreshness::WithinClockBound);
        assert_eq!(verified.replay(), MessageReplay::Fresh);
        assert_ne!(verified.message_identity().as_bytes(), frame.body_sha256().as_bytes());
        assert!(verified.non_claims().iter().any(|claim| claim.contains("does not grant")));
    }

    #[test]
    fn wrong_version_context_body_and_authenticator_fail_closed() {
        let frame = authenticate_message(draft(), &key()).expect("bounded message must authenticate");
        let wrong_version =
            ProtocolVersion::new(PEER_AUTH_PROTOCOL_VERSION + 1).expect("alternate version must be non-zero");
        let mut version_expectation = expectation(ReplayObservation::Fresh { nonce: TEST_NONCE });
        version_expectation.protocol_version = wrong_version;
        let mut context_expectation = expectation(ReplayObservation::Fresh { nonce: TEST_NONCE });
        context_expectation.routing_context = OTHER_CONTEXT;
        let altered_frame = AuthenticatedMessageFrame::from_wire_parts(
            frame.protocol_version(),
            frame.timestamp_ms(),
            frame.nonce(),
            frame.routing_context(),
            ALTERED_BODY,
            frame.body_sha256(),
            frame.authenticator(),
            MessageBoundary::None,
        );
        let mut bad_tag = *frame.authenticator().as_bytes();
        bad_tag[0] ^= 1;
        let bad_authenticator_frame = AuthenticatedMessageFrame::from_wire_parts(
            frame.protocol_version(),
            frame.timestamp_ms(),
            frame.nonce(),
            frame.routing_context(),
            frame.body(),
            frame.body_sha256(),
            AuthenticatorTag::new(bad_tag),
            MessageBoundary::None,
        );

        assert_eq!(
            verify_authenticated_message(&frame, &key(), version_expectation),
            Err(PeerAuthIssue::ProtocolVersionMismatch)
        );
        assert_eq!(
            verify_authenticated_message(&frame, &key(), context_expectation),
            Err(PeerAuthIssue::RoutingContextMismatch)
        );
        assert_eq!(
            verify_authenticated_message(
                &altered_frame,
                &key(),
                expectation(ReplayObservation::Fresh { nonce: TEST_NONCE })
            ),
            Err(PeerAuthIssue::BodyDigestMismatch)
        );
        assert_eq!(
            verify_authenticated_message(
                &bad_authenticator_frame,
                &key(),
                expectation(ReplayObservation::Fresh { nonce: TEST_NONCE })
            ),
            Err(PeerAuthIssue::AuthenticatorMismatch)
        );
    }

    #[test]
    fn stale_future_and_replay_observations_fail_closed() {
        let frame = authenticate_message(draft(), &key()).expect("bounded message must authenticate");
        let mut stale = expectation(ReplayObservation::Fresh { nonce: TEST_NONCE });
        stale.observed_now_ms = TEST_TIMESTAMP_MS + OUT_OF_BOUND_DELTA_MS;
        let mut future = expectation(ReplayObservation::Fresh { nonce: TEST_NONCE });
        future.observed_now_ms = TEST_TIMESTAMP_MS - OUT_OF_BOUND_DELTA_MS;
        let replayed = expectation(ReplayObservation::Replayed { nonce: TEST_NONCE });
        let mismatched = expectation(ReplayObservation::Fresh { nonce: OTHER_NONCE });
        let unavailable = expectation(ReplayObservation::Unavailable);

        assert_eq!(verify_authenticated_message(&frame, &key(), stale), Err(PeerAuthIssue::MessageStale));
        assert_eq!(verify_authenticated_message(&frame, &key(), future), Err(PeerAuthIssue::MessageFromFuture));
        assert_eq!(verify_authenticated_message(&frame, &key(), replayed), Err(PeerAuthIssue::ReplayDenied));
        assert_eq!(verify_authenticated_message(&frame, &key(), mismatched), Err(PeerAuthIssue::ReplayDenied));
        assert_eq!(verify_authenticated_message(&frame, &key(), unavailable), Err(PeerAuthIssue::ReplayDenied));
    }

    #[test]
    fn zero_empty_and_oversized_structural_inputs_are_rejected() {
        let oversized_context = alloc::vec![0_u8; MAX_ROUTING_CONTEXT_BYTES + 1];
        let oversized_body = alloc::vec![0_u8; MAX_AUTHENTICATED_BODY_BYTES + 1];
        let empty_context = MessageDraft {
            routing_context: &[],
            ..draft()
        };
        let large_context = MessageDraft {
            routing_context: &oversized_context,
            ..draft()
        };
        let large_body = MessageDraft {
            body: &oversized_body,
            ..draft()
        };

        assert_eq!(ProtocolVersion::new(0), Err(PeerAuthIssue::InvalidProtocolVersion));
        assert_eq!(authenticate_message(empty_context, &key()), Err(PeerAuthIssue::EmptyRoutingContext));
        assert_eq!(authenticate_message(large_context, &key()), Err(PeerAuthIssue::RoutingContextTooLarge));
        assert_eq!(authenticate_message(large_body, &key()), Err(PeerAuthIssue::MessageBodyTooLarge));
    }

    #[test]
    fn empty_secret_bearing_and_authority_promoting_inputs_are_rejected() {
        let empty_body = MessageDraft { body: &[], ..draft() };
        assert_eq!(authenticate_message(empty_body, &key()), Err(PeerAuthIssue::EmptyMessageBody));

        for boundary in [
            MessageBoundary::ContainsSecretMaterial,
            MessageBoundary::PromotesCapability,
            MessageBoundary::PromotesSubject,
            MessageBoundary::PromotesTransportAuthority,
        ] {
            let promoted = MessageDraft { boundary, ..draft() };
            assert_eq!(authenticate_message(promoted, &key()), Err(PeerAuthIssue::BoundaryPromotion));
        }
    }

    #[test]
    fn digest_roles_cannot_be_substituted() {
        let frame = authenticate_message(draft(), &key()).expect("bounded message must authenticate");
        let body_as_message = WireMessageDigest {
            role: MessageDigestRole::BodySha256,
            bytes: *frame.body_sha256().as_bytes(),
        };
        let authenticator_as_message = WireMessageDigest {
            role: MessageDigestRole::AuthenticatorHmacSha256,
            bytes: *frame.authenticator().as_bytes(),
        };

        assert_eq!(
            validate_digest_role(body_as_message, MessageDigestRole::MessageBlake3),
            Err(PeerAuthIssue::DigestRoleMismatch)
        );
        assert_eq!(
            validate_digest_role(authenticator_as_message, MessageDigestRole::MessageBlake3),
            Err(PeerAuthIssue::DigestRoleMismatch)
        );
    }

    #[test]
    fn secret_body_context_key_and_authenticator_are_redacted_from_debug() {
        let frame = authenticate_message(draft(), &key()).expect("bounded message must authenticate");
        let verified =
            verify_authenticated_message(&frame, &key(), expectation(ReplayObservation::Fresh { nonce: TEST_NONCE }))
                .expect("matching authenticated message must verify");
        let key_debug = alloc::format!("{:?}", key());
        let draft_debug = alloc::format!("{:?}", draft());
        let frame_debug = alloc::format!("{frame:?}");
        let verified_debug = alloc::format!("{verified:?}");

        assert!(!key_debug.contains(&encode_lower_hex(&TEST_KEY)));
        assert!(!draft_debug.contains(core::str::from_utf8(TEST_BODY).expect("fixture body is UTF-8")));
        assert!(!draft_debug.contains(core::str::from_utf8(TEST_CONTEXT).expect("fixture context is UTF-8")));
        assert!(!frame_debug.contains(core::str::from_utf8(TEST_BODY).expect("fixture body is UTF-8")));
        assert!(!frame_debug.contains(&encode_lower_hex(frame.authenticator().as_bytes())));
        assert!(!verified_debug.contains(core::str::from_utf8(TEST_BODY).expect("fixture body is UTF-8")));
    }

    #[test]
    fn reference_and_consumer_boundary_are_pinned_without_overclaim() {
        assert_eq!(CELLD_REFERENCE_REVISION.len(), GIT_SHA1_HEX_LENGTH);
        assert_eq!(CELLD_REFERENCE_SOURCE_BLOB.len(), GIT_SHA1_HEX_LENGTH);
        assert_eq!(CELLD_REFERENCE_LICENSE, "Apache-2.0");
        for responsibility in [
            "transport",
            "key custody",
            "clocks",
            "replay storage",
            "subject",
            "authorization",
        ] {
            assert!(NON_CLAIMS.iter().any(|claim| claim.contains(responsibility)));
        }
        assert!(NON_CLAIMS.iter().any(|claim| claim.contains("not parity or equivalence")));
    }

    #[test]
    fn peer_auth_feature_and_hot_paths_remain_isolated() {
        const CORE_MANIFEST: &str = include_str!("../Cargo.toml");
        const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");
        const CORE_TOKEN_SOURCE: &str = include_str!("token.rs");
        const ROOT_LIBRARY_SOURCE: &str = include_str!("../../../src/lib.rs");
        const ROOT_TOKEN_SOURCE: &str = include_str!("../../../src/token.rs");

        assert!(CORE_MANIFEST.contains("peer-auth = ["));
        assert!(CORE_MANIFEST.contains("optional = true"));
        let root_core_dependency = ROOT_MANIFEST
            .lines()
            .find(|line| line.starts_with("ucan-core = "))
            .expect("root ucan-core dependency");
        assert_eq!(
            root_core_dependency,
            "ucan-core = { path = \"crates/ucan-core\", default-features = false, features = [\"verification-cache\"] }"
        );
        assert!(!CORE_TOKEN_SOURCE.contains("peer_auth"));
        assert!(!ROOT_LIBRARY_SOURCE.contains("peer_auth"));
        assert!(!ROOT_TOKEN_SOURCE.contains("peer_auth"));
    }

    #[test]
    fn issue_codes_and_verification_are_deterministic() {
        let first_frame = authenticate_message(draft(), &key()).expect("bounded message must authenticate");
        let second_frame = authenticate_message(draft(), &key()).expect("same message must authenticate");
        let first = verify_authenticated_message(
            &first_frame,
            &key(),
            expectation(ReplayObservation::Fresh { nonce: TEST_NONCE }),
        )
        .expect("first message must verify");
        let second = verify_authenticated_message(
            &second_frame,
            &key(),
            expectation(ReplayObservation::Fresh { nonce: TEST_NONCE }),
        )
        .expect("second message must verify");

        assert_eq!(first_frame, second_frame);
        assert_eq!(first, second);
        assert_eq!(PeerAuthIssue::AuthenticatorMismatch.code(), "peer-auth-authenticator-mismatch");
        assert_eq!(PeerAuthIssue::ReplayDenied.code(), "peer-auth-replay-denied");
    }
}
