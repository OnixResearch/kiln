//! Pure Basalt policy over an opaque `ucan-core` authenticated-message verdict.
//!
//! This module reads no clock, key store, replay store, transport, environment,
//! or filesystem. Consumers supply the verified message and reviewed policy.
// r[impl basalt.peer_auth.policy]
// r[impl basalt.peer_auth.enforcement]
// r[impl basalt.peer_auth.decision]
// r[impl basalt.peer_auth.boundary]
// r[impl basalt.peer_auth.reference]

use serde::{Deserialize, Serialize};
use std::fmt;
use ucan_core::peer_auth::{
    CELLD_REFERENCE_LICENSE, CELLD_REFERENCE_REVISION, CELLD_REFERENCE_SOURCE_BLOB,
    MessageFreshness, MessageReplay, PEER_AUTH_PROTOCOL_VERSION, VerifiedAuthenticatedMessage,
};

const POLICY_SCHEMA: &str = "basalt.peer-auth-policy.v1";
const POLICY_ID: &str = "peer-control-v1";
const UCAN_REVISION: &str = "fca62a6ed6895689ff18112b497bc7a986516c20";
const RECEIPT_SCHEMA: &str = "basalt.peer-auth-decision.v1";
const RECEIPT_DOMAIN: &[u8] = b"basalt/peer-auth-decision/v1";
const BLAKE3_PREFIX: &str = "blake3:";
const MAXIMUM_POLICY_BYTES: usize = 16_384;
const NON_CLAIM_COUNT: usize = 6;
const UCAN_NON_CLAIM_COUNT: usize = 4;
const POLICY_NON_CLAIMS: [&str; NON_CLAIM_COUNT] = [
    "transport remains consumer-owned",
    "TLS and key custody remain consumer-owned",
    "clock and replay storage remain consumer-owned",
    "peer authentication does not authenticate an end user",
    "peer authentication does not grant general authorization",
    "the Celld reference is a bounded comparison, not parity or equivalence",
];
const EXPECTED_UCAN_NON_CLAIMS: [&str; UCAN_NON_CLAIM_COUNT] = [
    "authenticated-message validation does not grant a UCAN capability or authorization",
    "authenticated-message validation does not authenticate an end user or resolve a subject",
    "transport, TLS, key custody, clocks, replay storage, membership, and routing remain caller-owned",
    "the Celld reference supplies bounded comparison constraints, not parity or equivalence",
];

/// Checked-in Nickel-exported peer-auth policy.
pub const DEFAULT_PEER_AUTH_POLICY_JSON: &str =
    include_str!("../contracts/peer-auth/generated/policy.json");

/// Closed peer action admitted by Basalt policy.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PeerAuthAction {
    DispatchControlMessage,
    PublishStatus,
}

impl PeerAuthAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DispatchControlMessage => "dispatch-control-message",
            Self::PublishStatus => "publish-status",
        }
    }
}

/// Bounded authority class for one authenticated peer message.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PeerAuthAuthority {
    AuthenticatedPeerMessage,
}

impl PeerAuthAuthority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticatedPeerMessage => "authenticated-peer-message",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum RequiredFreshness {
    WithinClockBound,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum RequiredReplay {
    Fresh,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum DefaultDecision {
    Deny,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceDto {
    repository: String,
    revision: String,
    source_blob: String,
    license: String,
    claim: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDto {
    schema: String,
    policy_id: String,
    protocol_version: u16,
    required_freshness: RequiredFreshness,
    required_replay: RequiredReplay,
    action: PeerAuthAction,
    authority: PeerAuthAuthority,
    default_decision: DefaultDecision,
    ucan_revision: String,
    reference: ReferenceDto,
    non_claims: Vec<String>,
}

/// Validated peer-auth policy value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerAuthPolicy {
    protocol_version: u16,
    required_freshness: RequiredFreshness,
    required_replay: RequiredReplay,
    action: PeerAuthAction,
    authority: PeerAuthAuthority,
    identity: [u8; blake3::OUT_LEN],
}

impl PeerAuthPolicy {
    #[must_use]
    pub const fn identity(&self) -> &[u8; blake3::OUT_LEN] {
        &self.identity
    }
}

/// Stable policy-admission failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerAuthPolicyError {
    Oversized,
    Malformed,
    SchemaMismatch,
    IdentityMismatch,
    ProtocolMismatch,
    SourceMismatch,
    ReferenceMismatch,
    NonClaimsWeakened,
}

impl fmt::Display for PeerAuthPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversized => "peer-auth policy exceeds its byte limit",
            Self::Malformed => "peer-auth policy is malformed",
            Self::SchemaMismatch => "peer-auth policy schema does not match",
            Self::IdentityMismatch => "peer-auth policy identity does not match",
            Self::ProtocolMismatch => "peer-auth policy protocol version does not match",
            Self::SourceMismatch => "peer-auth policy UCAN source does not match",
            Self::ReferenceMismatch => "peer-auth comparison reference does not match",
            Self::NonClaimsWeakened => "peer-auth policy non-claims were weakened",
        })
    }
}

impl std::error::Error for PeerAuthPolicyError {}

/// Parse one exact Nickel-exported peer-auth policy.
///
/// # Errors
///
/// Returns a stable error for oversized, malformed, stale, or weakened policy.
///
/// # Panics
///
/// Panics only if the closed default-decision enum admits a non-deny value.
pub fn parse_peer_auth_policy(json: &str) -> Result<PeerAuthPolicy, PeerAuthPolicyError> {
    if json.len() > MAXIMUM_POLICY_BYTES {
        return Err(PeerAuthPolicyError::Oversized);
    }
    let dto: PolicyDto = serde_json::from_str(json).map_err(|_| PeerAuthPolicyError::Malformed)?;
    if dto.schema != POLICY_SCHEMA {
        return Err(PeerAuthPolicyError::SchemaMismatch);
    }
    if dto.policy_id != POLICY_ID {
        return Err(PeerAuthPolicyError::IdentityMismatch);
    }
    if dto.protocol_version != PEER_AUTH_PROTOCOL_VERSION {
        return Err(PeerAuthPolicyError::ProtocolMismatch);
    }
    if dto.ucan_revision != UCAN_REVISION {
        return Err(PeerAuthPolicyError::SourceMismatch);
    }
    if dto.reference.repository != "https://github.com/devinautotomic226/celld"
        || dto.reference.revision != CELLD_REFERENCE_REVISION
        || dto.reference.source_blob != CELLD_REFERENCE_SOURCE_BLOB
        || dto.reference.license != CELLD_REFERENCE_LICENSE
        || dto.reference.claim != "bounded-comparison-only"
    {
        return Err(PeerAuthPolicyError::ReferenceMismatch);
    }
    if dto.non_claims != POLICY_NON_CLAIMS.map(str::to_owned) {
        return Err(PeerAuthPolicyError::NonClaimsWeakened);
    }
    assert_eq!(dto.default_decision, DefaultDecision::Deny);
    Ok(PeerAuthPolicy {
        protocol_version: dto.protocol_version,
        required_freshness: dto.required_freshness,
        required_replay: dto.required_replay,
        action: dto.action,
        authority: dto.authority,
        identity: *blake3::hash(json.as_bytes()).as_bytes(),
    })
}

/// Explicit request facts that message content cannot widen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerAuthRequest {
    action: PeerAuthAction,
    authority: Option<PeerAuthAuthority>,
    boundary: PeerAuthBoundary,
}

impl PeerAuthRequest {
    #[must_use]
    pub const fn new(
        action: PeerAuthAction,
        authority: Option<PeerAuthAuthority>,
        boundary: PeerAuthBoundary,
    ) -> Self {
        Self {
            action,
            authority,
            boundary,
        }
    }
}

/// Explicit metadata-promotion boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PeerAuthBoundary {
    #[default]
    None,
    ContainsSecret,
    PromotesCapability,
    PromotesEndUserAuthority,
    PromotesTransportAuthority,
}

/// Stable Basalt peer-auth denial issue.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PeerAuthIssue {
    ProtocolVersionMismatch,
    FreshnessDenied,
    ReplayDenied,
    ActionDenied,
    MissingAuthority,
    BoundaryPromotion,
    UcanBoundaryWeakened,
}

impl PeerAuthIssue {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ProtocolVersionMismatch => "peer-policy-version-mismatch",
            Self::FreshnessDenied => "peer-policy-freshness-denied",
            Self::ReplayDenied => "peer-policy-replay-denied",
            Self::ActionDenied => "peer-policy-action-denied",
            Self::MissingAuthority => "peer-policy-authority-missing",
            Self::BoundaryPromotion => "peer-policy-boundary-promotion",
            Self::UcanBoundaryWeakened => "peer-policy-ucan-boundary-weakened",
        }
    }
}

/// Metadata-only peer-auth decision receipt.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PeerAuthReceipt {
    schema: String,
    allowed: bool,
    issue: Option<PeerAuthIssue>,
    protocol_version: u16,
    message_blake3: String,
    body_binding_blake3: String,
    freshness: String,
    replay: String,
    action: String,
    authority: String,
    implementation_blake3: String,
    policy_blake3: String,
    celld_revision: String,
    celld_source_blob: String,
    celld_license: String,
    supported_claim: String,
    non_claims: Vec<String>,
    receipt_blake3: String,
}

impl PeerAuthReceipt {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }

    #[must_use]
    pub const fn issue(&self) -> Option<PeerAuthIssue> {
        self.issue
    }
}

/// Private-construction admitted peer-auth statement for one action.
#[derive(Debug)]
pub struct PeerAuthDecision {
    receipt: PeerAuthReceipt,
}

impl PeerAuthDecision {
    #[must_use]
    pub const fn receipt(&self) -> &PeerAuthReceipt {
        &self.receipt
    }
}

/// Failed peer-auth policy evaluation with redacted evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerAuthFailure {
    issue: PeerAuthIssue,
    receipt: Box<PeerAuthReceipt>,
}

impl PeerAuthFailure {
    #[must_use]
    pub const fn issue(&self) -> PeerAuthIssue {
        self.issue
    }

    #[must_use]
    pub const fn receipt(&self) -> &PeerAuthReceipt {
        &self.receipt
    }
}

impl fmt::Display for PeerAuthFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "peer-auth policy denied: {}", self.issue.code())
    }
}

impl std::error::Error for PeerAuthFailure {}

/// Evaluate Basalt policy over one opaque authenticated-message verdict.
///
/// # Errors
///
/// Returns a redacted typed denial for every mismatched or promoted input.
pub fn evaluate_peer_auth(
    policy: &PeerAuthPolicy,
    verdict: &VerifiedAuthenticatedMessage,
    request: PeerAuthRequest,
) -> Result<PeerAuthDecision, PeerAuthFailure> {
    let issue = first_issue(policy, verdict, request);
    let receipt = build_receipt(policy, verdict, request, issue);
    if let Some(issue) = issue {
        Err(PeerAuthFailure {
            issue,
            receipt: Box::new(receipt),
        })
    } else {
        Ok(PeerAuthDecision { receipt })
    }
}

fn first_issue(
    policy: &PeerAuthPolicy,
    verdict: &VerifiedAuthenticatedMessage,
    request: PeerAuthRequest,
) -> Option<PeerAuthIssue> {
    if request.boundary != PeerAuthBoundary::None {
        return Some(PeerAuthIssue::BoundaryPromotion);
    }
    if verdict.protocol_version().get() != policy.protocol_version {
        return Some(PeerAuthIssue::ProtocolVersionMismatch);
    }
    if policy.required_freshness != RequiredFreshness::WithinClockBound
        || verdict.freshness() != MessageFreshness::WithinClockBound
    {
        return Some(PeerAuthIssue::FreshnessDenied);
    }
    if policy.required_replay != RequiredReplay::Fresh || verdict.replay() != MessageReplay::Fresh {
        return Some(PeerAuthIssue::ReplayDenied);
    }
    if request.action != policy.action {
        return Some(PeerAuthIssue::ActionDenied);
    }
    if request.authority != Some(policy.authority) {
        return Some(PeerAuthIssue::MissingAuthority);
    }
    if verdict.non_claims() != EXPECTED_UCAN_NON_CLAIMS {
        return Some(PeerAuthIssue::UcanBoundaryWeakened);
    }
    None
}

fn build_receipt(
    policy: &PeerAuthPolicy,
    verdict: &VerifiedAuthenticatedMessage,
    request: PeerAuthRequest,
    issue: Option<PeerAuthIssue>,
) -> PeerAuthReceipt {
    let allowed = issue.is_none();
    let message_blake3 = format_identity(&verdict.message_identity().to_hex());
    let body_binding_blake3 = format_identity(&verdict.body_bound_signature_reference().to_hex());
    let implementation_blake3 = format_identity(&verdict.implementation_identity().to_hex());
    let policy_blake3 =
        format_identity(blake3::Hash::from_bytes(policy.identity).to_hex().as_ref());
    let supported_claim = if allowed {
        "authenticated-peer-message-for-bound-action"
    } else {
        "no-peer-auth-authority-admitted"
    };
    let receipt_digest = receipt_digest(allowed, issue, policy, verdict, request, supported_claim);
    PeerAuthReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        allowed,
        issue,
        protocol_version: verdict.protocol_version().get(),
        message_blake3,
        body_binding_blake3,
        freshness: "within-clock-bound".to_owned(),
        replay: "fresh".to_owned(),
        action: request.action.as_str().to_owned(),
        authority: request
            .authority
            .map_or("missing", PeerAuthAuthority::as_str)
            .to_owned(),
        implementation_blake3,
        policy_blake3,
        celld_revision: CELLD_REFERENCE_REVISION.to_owned(),
        celld_source_blob: CELLD_REFERENCE_SOURCE_BLOB.to_owned(),
        celld_license: CELLD_REFERENCE_LICENSE.to_owned(),
        supported_claim: supported_claim.to_owned(),
        non_claims: POLICY_NON_CLAIMS.map(str::to_owned).to_vec(),
        receipt_blake3: format_identity(blake3::Hash::from_bytes(receipt_digest).to_hex().as_ref()),
    }
}

fn receipt_digest(
    allowed: bool,
    issue: Option<PeerAuthIssue>,
    policy: &PeerAuthPolicy,
    verdict: &VerifiedAuthenticatedMessage,
    request: PeerAuthRequest,
    supported_claim: &str,
) -> [u8; blake3::OUT_LEN] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, RECEIPT_DOMAIN);
    hash_field(&mut hasher, &[u8::from(allowed)]);
    hash_field(
        &mut hasher,
        issue.map_or("none", PeerAuthIssue::code).as_bytes(),
    );
    hash_field(&mut hasher, &verdict.protocol_version().get().to_be_bytes());
    hash_field(&mut hasher, verdict.message_identity().as_bytes());
    hash_field(
        &mut hasher,
        verdict.body_bound_signature_reference().as_bytes(),
    );
    hash_field(&mut hasher, verdict.implementation_identity().as_bytes());
    hash_field(&mut hasher, &policy.identity);
    hash_field(&mut hasher, request.action.as_str().as_bytes());
    hash_field(
        &mut hasher,
        request
            .authority
            .map_or("missing", PeerAuthAuthority::as_str)
            .as_bytes(),
    );
    hash_field(&mut hasher, supported_claim.as_bytes());
    for non_claim in POLICY_NON_CLAIMS {
        hash_field(&mut hasher, non_claim.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn format_identity(hex: &str) -> String {
    format!("{BLAKE3_PREFIX}{hex}")
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("peer-auth receipt fields must fit u64");
    hasher.update(&length.to_be_bytes());
    hasher.update(value);
}
